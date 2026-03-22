//! MCP subprocess client for communicating with sibling ecosystem tools.
//!
//! Spawns a tool as a subprocess, sends JSON-RPC requests over stdin,
//! and reads responses from stdout.
//!
//! Supports two framing modes:
//! - `LineDelimited` (default): newline-delimited JSON, used by Hyphae and Rhizome
//! - `ContentLength`: LSP-style headers + body, used by LSP servers

use crate::discovery::discover;
use crate::error::{Result, SporeError};
use crate::jsonrpc;
use crate::types::Tool;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Default)]
pub enum Framing {
    /// ─────────────────────────────────────────────────────────────────────────
    /// `LineDelimited`
    /// ─────────────────────────────────────────────────────────────────────────
    /// Newline-delimited JSON. Each message is a complete JSON object on a
    /// single line, terminated by \n. Default for ecosystem MCP servers.
    #[default]
    LineDelimited,

    /// ─────────────────────────────────────────────────────────────────────────
    /// `ContentLength`
    /// ─────────────────────────────────────────────────────────────────────────
    /// LSP-style Content-Length headers followed by a blank line, then the body.
    /// Used by LSP servers.
    ContentLength,
}

/// ─────────────────────────────────────────────────────────────────────────
/// MCP Client
/// ─────────────────────────────────────────────────────────────────────────
/// MCP subprocess client. NOT thread-safe — must be used from a single thread.
/// Use separate `McpClient` instances for concurrent access to the same tool.
pub struct McpClient {
    tool: Tool,
    args: Vec<String>,
    child: Option<Child>,
    timeout: Duration,
    framing: Framing,
}

impl McpClient {
    /// Spawn a new MCP client for the given tool.
    ///
    /// Defaults to `Framing::LineDelimited` for compatibility with Hyphae and Rhizome.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool binary is not found in PATH or cannot be spawned.
    pub fn spawn(tool: Tool, args: &[&str]) -> Result<Self> {
        let mut client = Self {
            tool,
            args: args.iter().map(|&s| s.to_string()).collect(),
            child: None,
            timeout: DEFAULT_TIMEOUT,
            framing: Framing::default(),
        };
        client.ensure_alive()?;
        Ok(client)
    }

    /// Set the timeout for tool calls.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the framing mode for this client.
    ///
    /// Default is `Framing::LineDelimited` for compatibility with ecosystem MCP servers.
    #[must_use]
    pub fn with_framing(mut self, framing: Framing) -> Self {
        self.framing = framing;
        self
    }

    /// Call an MCP tool and return the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the subprocess is not running, the request fails to send,
    /// the response is malformed, or the server returns a JSON-RPC error.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "ergonomic API: callers pass json!({...}) directly"
    )]
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        self.ensure_alive()?;

        let request = jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": name,
                "arguments": arguments,
            }),
        );

        let encoded = jsonrpc::encode(&request);
        let _child = self
            .child
            .as_mut()
            .ok_or_else(|| SporeError::Other("No child process".to_string()))?;

        // ─────────────────────────────────────────────────────────────────────
        // Write Request
        // ─────────────────────────────────────────────────────────────────────
        self.send_request(&encoded)?;

        // ─────────────────────────────────────────────────────────────────────
        // Read Response
        // ─────────────────────────────────────────────────────────────────────
        let response = self.recv_response()?;

        if let Some(error) = response.error {
            return Err(SporeError::RpcError {
                code: error.code,
                message: error.message,
            });
        }

        response
            .result
            .ok_or_else(|| SporeError::Other("Empty result in response".to_string()))
    }

    /// ─────────────────────────────────────────────────────────────────────────
    /// Send Request
    /// ─────────────────────────────────────────────────────────────────────────
    fn send_request(&mut self, encoded: &str) -> Result<()> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| SporeError::Other("No child process".to_string()))?;
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| SporeError::Other("No stdin".to_string()))?;

        match self.framing {
            Framing::LineDelimited => {
                // Write JSON object + newline
                stdin
                    .write_all(encoded.as_bytes())
                    .map_err(SporeError::SpawnFailed)?;
                stdin.write_all(b"\n").map_err(SporeError::SpawnFailed)?;
            }
            Framing::ContentLength => {
                // Write as LSP-style: Content-Length header + blank line + body
                let header = format!("Content-Length: {}\r\n\r\n", encoded.len());
                stdin
                    .write_all(header.as_bytes())
                    .map_err(SporeError::SpawnFailed)?;
                stdin
                    .write_all(encoded.as_bytes())
                    .map_err(SporeError::SpawnFailed)?;
            }
        }

        stdin.flush().map_err(SporeError::SpawnFailed)?;
        Ok(())
    }

    /// ─────────────────────────────────────────────────────────────────────────
    /// Recv Response
    /// ─────────────────────────────────────────────────────────────────────────
    /// Reads response from subprocess stdout with proper timeout enforcement.
    /// Uses a separate thread for the blocking read, allowing the main thread
    /// to enforce the timeout. If the timeout expires, the child process is
    /// killed and an error is returned.
    fn recv_response(&mut self) -> Result<jsonrpc::Response> {
        let framing = self.framing;
        let timeout = self.timeout;

        let child = self
            .child
            .as_mut()
            .ok_or_else(|| SporeError::Other("No child process".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SporeError::Other("No stdout".to_string()))?;

        let (tx, rx) = std::sync::mpsc::channel();

        // Spawn a thread to perform the blocking read
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let result = match framing {
                Framing::LineDelimited => read_line_delimited(&mut reader),
                Framing::ContentLength => read_content_length(&mut reader),
            };
            // Extract stdout and send both back through channel
            let stdout_back = reader.into_inner();
            let _ = tx.send((result, stdout_back));
        });

        if let Ok((result, stdout_back)) = rx.recv_timeout(timeout) {
            // Put stdout back so child can be reused
            if let Some(child) = self.child.as_mut() {
                child.stdout = Some(stdout_back);
            }
            result
        } else {
            // Timeout expired — kill the child process.
            // The reader thread is blocked on ChildStdout::read. When we kill the child below,
            // its stdout closes, unblocking the thread. The thread then sends on a disconnected
            // channel (tx dropped) and exits. No thread leak occurs in practice.
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
            }
            Err(SporeError::Timeout(timeout))
        }
    }

    /// Check if the subprocess is still running.
    #[must_use]
    pub fn is_alive(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|c| c.try_wait().ok().flatten().is_none())
    }

    fn ensure_alive(&mut self) -> Result<()> {
        if self.is_alive() {
            return Ok(());
        }

        // Kill old process if it exists
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }

        let info =
            discover(self.tool).ok_or_else(|| SporeError::ToolNotFound(self.tool.to_string()))?;

        let child = Command::new(&info.binary_path)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(SporeError::SpawnFailed)?;

        self.child = Some(child);
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

/// ─────────────────────────────────────────────────────────────────────────
/// Read Line Delimited
/// ─────────────────────────────────────────────────────────────────────────
/// Read a single line and parse it as JSON.
fn read_line_delimited(
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<jsonrpc::Response> {
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .map_err(SporeError::SpawnFailed)?;

    if n == 0 {
        return Err(SporeError::Other("EOF while reading response".to_string()));
    }

    serde_json::from_str(line.trim()).map_err(SporeError::Json)
}

/// ─────────────────────────────────────────────────────────────────────────
/// Read Content Length
/// ─────────────────────────────────────────────────────────────────────────
/// Read Content-Length headers, skip blank line, then read body.
fn read_content_length(
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<jsonrpc::Response> {
    let mut content_length = 0;

    // Read headers
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(SporeError::SpawnFailed)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(len) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len
                .parse()
                .map_err(|_| SporeError::Other("Invalid Content-Length".to_string()))?;
        }
    }

    if content_length == 0 {
        return Err(SporeError::Other(
            "No Content-Length in response".to_string(),
        ));
    }

    // Read body
    let mut body = vec![0u8; content_length];
    std::io::Read::read_exact(reader, &mut body).map_err(SporeError::SpawnFailed)?;
    let body_str = String::from_utf8(body)
        .map_err(|_| SporeError::Other("Invalid UTF-8 in response body".to_string()))?;

    serde_json::from_str(&body_str).map_err(SporeError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build an `McpClient` without spawning a real ecosystem tool.
    /// Uses `child: None` so no subprocess is involved.
    fn stub_client() -> McpClient {
        McpClient {
            tool: Tool::Hyphae,
            args: vec!["serve".to_string()],
            child: None,
            timeout: Duration::from_secs(5),
            framing: Framing::LineDelimited,
        }
    }

    /// Helper: build an `McpClient` backed by a Python mock MCP server using
    /// line-delimited JSON (newline-separated messages).
    ///
    /// The mock reads until it sees a newline, then writes a canned JSON-RPC
    /// response as a line, and blocks until killed.
    fn mock_server_line_delimited() -> McpClient {
        let script = r#"
import sys
# Read until we see newline (end of line-delimited JSON)
line = sys.stdin.readline()
# Write response as newline-delimited JSON
resp = '{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"}]}}'
sys.stdout.write(resp + '\n')
sys.stdout.flush()
# Block until killed
sys.stdin.read()
"#;
        let child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn mock MCP server (python3 required)");

        McpClient {
            tool: Tool::Hyphae,
            args: vec![],
            child: Some(child),
            timeout: Duration::from_secs(5),
            framing: Framing::LineDelimited,
        }
    }

    /// Helper: build an `McpClient` backed by a Python mock MCP server using
    /// Content-Length framing (LSP-style headers + body).
    ///
    /// The mock reads until it sees a closing brace (end of JSON body),
    /// then writes a canned JSON-RPC response with Content-Length headers.
    fn mock_server_content_length() -> McpClient {
        let script = r#"
import sys
# Read until we see closing brace (end of JSON body)
while True:
    ch = sys.stdin.read(1)
    if not ch or ch == '}':
        break
# Write response with Content-Length headers
resp = '{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"}]}}'
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
# Block until killed
sys.stdin.read()
"#;
        let child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn mock MCP server (python3 required)");

        McpClient {
            tool: Tool::Hyphae,
            args: vec![],
            child: Some(child),
            timeout: Duration::from_secs(5),
            framing: Framing::ContentLength,
        }
    }

    #[test]
    fn test_is_alive_without_child() {
        let mut client = stub_client();
        assert!(!client.is_alive());
    }

    #[test]
    fn test_drop_does_not_panic_with_none_child() {
        let client = stub_client();
        drop(client);
    }

    #[test]
    fn test_drop_does_not_panic_with_live_child_line_delimited() {
        let client = mock_server_line_delimited();
        drop(client); // Should kill child cleanly
    }

    #[test]
    fn test_drop_does_not_panic_with_live_child_content_length() {
        let client = mock_server_content_length();
        drop(client); // Should kill child cleanly
    }

    #[test]
    fn test_is_alive_with_running_child_line_delimited() {
        let mut client = mock_server_line_delimited();
        assert!(client.is_alive());
    }

    #[test]
    fn test_is_alive_with_running_child_content_length() {
        let mut client = mock_server_content_length();
        assert!(client.is_alive());
    }

    #[test]
    fn test_with_timeout_returns_self() {
        let client = stub_client();
        let updated = client.with_timeout(Duration::from_secs(30));
        assert_eq!(updated.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_with_framing_returns_self() {
        let client = stub_client();
        let updated = client.with_framing(Framing::ContentLength);
        assert!(matches!(updated.framing, Framing::ContentLength));
    }

    #[test]
    fn test_call_tool_on_mock_server_line_delimited() {
        let mut client = mock_server_line_delimited();
        let result = client.call_tool("test_tool", serde_json::json!({"key": "value"}));
        assert!(result.is_ok(), "call_tool failed: {result:?}");

        let value = result.unwrap();
        // Mock server returns {"content":[{"type":"text","text":"ok"}]}
        let content = value.get("content").expect("missing content field");
        let first = content
            .as_array()
            .expect("content not array")
            .first()
            .unwrap();
        assert_eq!(first.get("text").and_then(|v| v.as_str()), Some("ok"));
    }

    #[test]
    fn test_call_tool_on_mock_server_content_length() {
        let mut client = mock_server_content_length();
        let result = client.call_tool("test_tool", serde_json::json!({"key": "value"}));
        assert!(result.is_ok(), "call_tool failed: {result:?}");

        let value = result.unwrap();
        // Mock server returns {"content":[{"type":"text","text":"ok"}]}
        let content = value.get("content").expect("missing content field");
        let first = content
            .as_array()
            .expect("content not array")
            .first()
            .unwrap();
        assert_eq!(first.get("text").and_then(|v| v.as_str()), Some("ok"));
    }

    #[test]
    fn test_ensure_alive_replaces_exited_child() {
        // Spawn a child that exits immediately
        let child = Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn 'true'");

        let mut client = McpClient {
            tool: Tool::Hyphae,
            args: vec![],
            child: Some(child),
            timeout: Duration::from_secs(1),
            framing: Framing::LineDelimited,
        };

        // Give the child time to exit
        std::thread::sleep(Duration::from_millis(50));

        // is_alive should return false after the child exits
        assert!(!client.is_alive());
    }

    #[test]
    fn test_timeout_kills_hung_subprocess_line_delimited() {
        // Create a mock server that never responds (blocks forever)
        let script = r#"
import sys
# Read request
line = sys.stdin.readline()
# Don't write response - just block forever
sys.stdin.read()
"#;
        let child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn mock server");

        let mut client = McpClient {
            tool: Tool::Hyphae,
            args: vec![],
            child: Some(child),
            timeout: Duration::from_millis(200), // Short timeout
            framing: Framing::LineDelimited,
        };

        // Send a request that will never get a response
        let request = jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": "test_tool",
                "arguments": {},
            }),
        );
        let encoded = jsonrpc::encode(&request);
        client.send_request(&encoded).expect("send_request failed");

        // recv_response should timeout and kill the child
        let result = client.recv_response();
        assert!(result.is_err(), "Expected timeout error");
        assert!(
            result.unwrap_err().to_string().contains("timeout"),
            "Expected timeout message"
        );

        // Child should be dead after timeout
        assert!(!client.is_alive(), "Child should be killed after timeout");
    }

    #[test]
    fn test_timeout_kills_hung_subprocess_content_length() {
        // Create a mock server that never responds (blocks forever)
        let script = r#"
import sys
# Read request character by character until closing brace
while True:
    ch = sys.stdin.read(1)
    if not ch or ch == '}':
        break
# Don't write response - just block forever
sys.stdin.read()
"#;
        let child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn mock server");

        let mut client = McpClient {
            tool: Tool::Hyphae,
            args: vec![],
            child: Some(child),
            timeout: Duration::from_millis(200), // Short timeout
            framing: Framing::ContentLength,
        };

        // Send a request that will never get a response
        let request = jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": "test_tool",
                "arguments": {},
            }),
        );
        let encoded = jsonrpc::encode(&request);
        client.send_request(&encoded).expect("send_request failed");

        // recv_response should timeout and kill the child
        let result = client.recv_response();
        assert!(result.is_err(), "Expected timeout error");
        assert!(
            result.unwrap_err().to_string().contains("timeout"),
            "Expected timeout message"
        );

        // Child should be dead after timeout
        assert!(!client.is_alive(), "Child should be killed after timeout");
    }
}

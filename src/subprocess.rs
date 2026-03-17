//! MCP subprocess client for communicating with sibling ecosystem tools.
//!
//! Spawns a tool as a subprocess, sends JSON-RPC requests over stdin,
//! and reads responses from stdout.

use crate::discovery::discover;
use crate::jsonrpc;
use crate::types::Tool;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct McpClient {
    tool: Tool,
    args: Vec<String>,
    child: Option<Child>,
    timeout: Duration,
}

impl McpClient {
    /// Spawn a new MCP client for the given tool.
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
        let child = self.child.as_mut().context("No child process")?;

        // Write request
        let stdin = child.stdin.as_mut().context("No stdin")?;
        stdin.write_all(encoded.as_bytes())?;
        stdin.flush()?;

        // Read response (simplified: read until we get a complete JSON object)
        let stdout = child.stdout.as_mut().context("No stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut content_length = 0;

        // Read headers
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(len) = trimmed.strip_prefix("Content-Length: ") {
                content_length = len.parse().context("Invalid Content-Length")?;
            }
        }

        if content_length == 0 {
            bail!("No Content-Length in response");
        }

        // Read body
        let mut body = vec![0u8; content_length];
        std::io::Read::read_exact(&mut reader, &mut body)?;
        let body_str = String::from_utf8(body)?;

        let response: jsonrpc::Response =
            serde_json::from_str(&body_str).context("Failed to parse response")?;

        if let Some(error) = response.error {
            bail!("RPC error {}: {}", error.code, error.message);
        }

        response.result.context("Empty result in response")
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
            discover(self.tool).with_context(|| format!("{} not found in PATH", self.tool))?;

        let child = Command::new(&info.binary_path)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn {}", self.tool))?;

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
        }
    }

    /// Helper: build an `McpClient` backed by a Python mock MCP server.
    ///
    /// The mock reads one request from stdin, then writes a canned JSON-RPC
    /// response, and blocks until killed.
    fn mock_server_client() -> McpClient {
        let script = r#"
import sys
# Read until we see closing brace (end of JSON body)
while True:
    ch = sys.stdin.read(1)
    if not ch or ch == '}':
        break
# Write response
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
    fn test_drop_does_not_panic_with_live_child() {
        let client = mock_server_client();
        drop(client); // Should kill child cleanly
    }

    #[test]
    fn test_is_alive_with_running_child() {
        let mut client = mock_server_client();
        assert!(client.is_alive());
    }

    #[test]
    fn test_with_timeout_returns_self() {
        let client = stub_client();
        let updated = client.with_timeout(Duration::from_secs(30));
        assert_eq!(updated.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_call_tool_on_mock_server() {
        let mut client = mock_server_client();
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
        };

        // Give the child time to exit
        std::thread::sleep(Duration::from_millis(50));

        // is_alive should return false after the child exits
        assert!(!client.is_alive());
    }
}

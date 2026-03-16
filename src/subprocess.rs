//! MCP subprocess client for communicating with sibling ecosystem tools.
//!
//! Spawns a tool as a subprocess, sends JSON-RPC requests over stdin,
//! and reads responses from stdout.

use crate::discovery::discover;
use crate::jsonrpc;
use crate::types::Tool;
use anyhow::{bail, Context, Result};
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
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Call an MCP tool and return the result.
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

        let info = discover(self.tool)
            .with_context(|| format!("{} not found in PATH", self.tool))?;

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

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl Request {
    #[must_use]
    pub fn new(method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            method: method.to_string(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Encode a JSON-RPC request with Content-Length header (MCP/LSP framing).
///
/// # Panics
///
/// Panics if the request cannot be serialized to JSON (should never happen
/// with valid `Request` structs).
#[must_use]
pub fn encode(request: &Request) -> String {
    let json = serde_json::to_string(request).expect("Request serialization cannot fail");
    format!("Content-Length: {}\r\n\r\n{json}", json.len())
}

/// Decode a JSON-RPC response from a Content-Length framed message.
/// Handles both `\r\n\r\n` (standard) and `\n\n` (common in practice) separators.
///
/// # Errors
///
/// Returns an error if the body cannot be parsed as a JSON-RPC response.
pub fn decode(input: &str) -> Result<Response> {
    let body = if let Some(idx) = input.find("\r\n\r\n") {
        &input[idx + 4..]
    } else if let Some(idx) = input.find("\n\n") {
        &input[idx + 2..]
    } else {
        input
    };
    serde_json::from_str(body).context("Failed to parse JSON-RPC response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let req = Request::new("tools/list", Value::Null);
        let encoded = encode(&req);

        assert!(encoded.starts_with("Content-Length:"));
        assert!(encoded.contains("tools/list"));
    }

    #[test]
    fn test_decode_response() {
        let raw = r#"Content-Length: 52

{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let resp = decode(raw).unwrap();
        assert_eq!(resp.id, 1);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_decode_bare_json() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        let resp = decode(raw).unwrap();
        assert_eq!(resp.id, 1);
    }

    #[test]
    fn test_decode_error_response() {
        let raw =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let resp = decode(raw).unwrap();
        assert!(resp.error.is_some());
        let error = resp.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
        assert!(resp.result.is_none());
    }

    #[test]
    fn test_decode_error_response_with_data() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request","data":"extra info"}}"#;
        let resp = decode(raw).unwrap();
        let error = resp.error.unwrap();
        assert_eq!(error.code, -32600);
        assert!(error.data.is_some());
    }

    #[test]
    fn test_decode_crlf_separator() {
        let raw = "Content-Length: 37\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}";
        let resp = decode(raw).unwrap();
        assert_eq!(resp.id, 1);
    }
}

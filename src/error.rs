//! Error types for the spore library.
//!
//! Provides typed errors for all public APIs so consumers can match on
//! specific error variants.

use crate::types::Tool;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ─────────────────────────────────────────────────────────────────────────
/// Spore Error
/// ─────────────────────────────────────────────────────────────────────────
/// Comprehensive error type for all spore operations.
#[derive(Debug, Error)]
pub enum SporeError {
    /// Tool not found in PATH.
    #[error("tool not found in PATH: {0}")]
    ToolNotFound(String),

    /// Failed to spawn a subprocess.
    #[error("failed to spawn process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// JSON-RPC protocol error from server.
    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i64, message: String },

    /// Subprocess communication timeout.
    #[error("response timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// Configuration loading or validation error.
    #[error("config error: {0}")]
    Config(String),

    /// Path resolution error.
    #[error("path error: {0}")]
    Path(String),

    /// Network error (HTTP, DNS, etc.).
    #[error("network error: {0}")]
    Network(String),

    /// JSON parsing or serialization error.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing or serialization error.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Generic error message.
    #[error("{0}")]
    Other(String),
}

pub const ECOSYSTEM_ERROR_SCHEMA_VERSION: &str = "1.0";

/// Serializable cross-tool error envelope for ecosystem boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcosystemError {
    pub schema_version: String,
    pub tool: Tool,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<EcosystemError>>,
}

impl EcosystemError {
    #[must_use]
    pub fn new(tool: Tool, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: ECOSYSTEM_ERROR_SCHEMA_VERSION.to_string(),
            tool,
            code: code.into(),
            message: message.into(),
            cause: None,
        }
    }

    #[must_use]
    pub fn with_cause(mut self, cause: EcosystemError) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }

    #[must_use]
    pub fn from_spore_error(tool: Tool, error: &SporeError) -> Self {
        Self::new(tool, spore_error_code(error), error.to_string())
    }

    #[must_use]
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"schema_version":"{ECOSYSTEM_ERROR_SCHEMA_VERSION}","tool":"{tool}","code":"serialization_error","message":"failed to serialize ecosystem error envelope"}}"#,
                tool = self.tool
            )
        })
    }
}

fn spore_error_code(error: &SporeError) -> &'static str {
    match error {
        SporeError::ToolNotFound(_) => "tool_not_found",
        SporeError::SpawnFailed(_) => "spawn_failed",
        SporeError::RpcError { .. } => "rpc_error",
        SporeError::Timeout(_) => "timeout",
        SporeError::Config(_) => "config_error",
        SporeError::Path(_) => "path_error",
        SporeError::Network(_) => "network_error",
        SporeError::Json(_) => "json_error",
        SporeError::Toml(_) => "toml_error",
        SporeError::Other(_) => "other",
    }
}

/// ─────────────────────────────────────────────────────────────────────────
/// Result Type Alias
/// ─────────────────────────────────────────────────────────────────────────
/// Convenient alias for `Result<T, SporeError>`.
pub type Result<T> = std::result::Result<T, SporeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_error_serializes_with_schema_version() {
        let err = EcosystemError::new(Tool::Hyphae, "tool_error", "Hyphae tool call failed");
        let parsed: serde_json::Value =
            serde_json::from_str(&err.to_json_string()).expect("valid error json");

        assert_eq!(parsed["schema_version"].as_str(), Some("1.0"));
        assert_eq!(parsed["tool"].as_str(), Some("hyphae"));
        assert_eq!(parsed["code"].as_str(), Some("tool_error"));
        assert_eq!(parsed["message"].as_str(), Some("Hyphae tool call failed"));
    }

    #[test]
    fn ecosystem_error_wraps_spore_error_with_cause_code() {
        let cause = EcosystemError::from_spore_error(
            Tool::Hyphae,
            &SporeError::RpcError {
                code: -32600,
                message: "Invalid request".to_string(),
            },
        );
        let err = EcosystemError::new(Tool::Mycelium, "call_failed", "Hyphae bridge failed")
            .with_cause(cause);
        let parsed: serde_json::Value =
            serde_json::from_str(&err.to_json_string()).expect("valid error json");

        assert_eq!(parsed["tool"].as_str(), Some("mycelium"));
        assert_eq!(parsed["cause"]["tool"].as_str(), Some("hyphae"));
        assert_eq!(parsed["cause"]["code"].as_str(), Some("rpc_error"));
    }
}

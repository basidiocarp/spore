//! Error types for the spore library.
//!
//! Provides typed errors for all public APIs so consumers can match on
//! specific error variants.

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

/// ─────────────────────────────────────────────────────────────────────────
/// Result Type Alias
/// ─────────────────────────────────────────────────────────────────────────
/// Convenient alias for `Result<T, SporeError>`.
pub type Result<T> = std::result::Result<T, SporeError>;

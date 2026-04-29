pub mod availability;
pub mod capability;
pub mod config;
pub mod datetime;
pub mod discovery;
pub mod editors;
pub mod error;
pub mod jsonrpc;
#[cfg(feature = "logging")]
pub mod logging;
pub mod paths;
#[cfg(feature = "http")]
pub mod self_update;
pub mod subprocess;
#[cfg(feature = "otel")]
pub mod telemetry;
pub mod tokens;
pub mod transport;
pub mod types;

pub use discovery::{discover, discover_all};
pub use error::{EcosystemError, Result, SporeError};
pub use subprocess::{Framing, McpClient};
pub use transport::{LocalServiceClient, LocalServiceEndpoint, TransportError};
pub use types::{EcosystemStatus, ProjectContext, Tool, ToolInfo};

/// Return a normalized runtime session id from `CLAUDE_SESSION_ID`.
///
/// Empty and whitespace-only values are treated as missing.
#[must_use]
pub fn claude_session_id() -> Option<String> {
    std::env::var("CLAUDE_SESSION_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

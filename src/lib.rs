pub mod config;
pub mod datetime;
pub mod discovery;
pub mod editors;
pub mod error;
pub mod jsonrpc;
pub mod logging;
pub mod paths;
pub mod self_update;
pub mod subprocess;
pub mod tokens;
pub mod types;

pub use discovery::{discover, discover_all};
pub use error::{EcosystemError, Result, SporeError};
pub use subprocess::{Framing, McpClient};
pub use types::{EcosystemStatus, ProjectContext, Tool, ToolInfo};

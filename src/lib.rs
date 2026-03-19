pub mod discovery;
pub mod jsonrpc;
pub mod subprocess;
pub mod types;

pub use discovery::{discover, discover_all};
pub use subprocess::{Framing, McpClient};
pub use types::{EcosystemStatus, ProjectContext, Tool, ToolInfo};

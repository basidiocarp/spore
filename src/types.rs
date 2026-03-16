use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Mycelium,
    Hyphae,
    Rhizome,
}

impl Tool {
    #[must_use]
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Mycelium => "mycelium",
            Self::Hyphae => "hyphae",
            Self::Rhizome => "rhizome",
        }
    }

    #[must_use]
    pub fn all() -> &'static [Tool] {
        &[Self::Mycelium, Self::Hyphae, Self::Rhizome]
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.binary_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub tool: Tool,
    pub binary_path: PathBuf,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemStatus {
    pub tools: Vec<ToolInfo>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub name: String,
    pub root: PathBuf,
    pub detected_languages: Vec<String>,
}

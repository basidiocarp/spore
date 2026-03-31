use crate::types::{Tool, ToolInfo};
use std::sync::OnceLock;

static MYCELIUM_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();
static HYPHAE_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();
static RHIZOME_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();
static CORTINA_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();
static CANOPY_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();
static CAP_CACHE: OnceLock<Option<ToolInfo>> = OnceLock::new();

/// Discover a specific ecosystem tool in PATH.
/// Results are cached for the lifetime of the process.
#[must_use]
pub fn discover(tool: Tool) -> Option<ToolInfo> {
    let cache = match tool {
        Tool::Mycelium => &MYCELIUM_CACHE,
        Tool::Hyphae => &HYPHAE_CACHE,
        Tool::Rhizome => &RHIZOME_CACHE,
        Tool::Cortina => &CORTINA_CACHE,
        Tool::Canopy => &CANOPY_CACHE,
        Tool::Cap => &CAP_CACHE,
    };
    cache.get_or_init(|| probe(tool)).clone()
}

/// Discover all ecosystem tools in PATH.
#[must_use]
pub fn discover_all() -> Vec<ToolInfo> {
    Tool::all().iter().filter_map(|&t| discover(t)).collect()
}

fn probe(tool: Tool) -> Option<ToolInfo> {
    let binary_path = which::which(tool.binary_name()).ok()?;

    let output = std::process::Command::new(&binary_path)
        .arg("--version")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = parse_version(&stdout).unwrap_or_default();

    Some(ToolInfo {
        tool,
        binary_path,
        version,
    })
}

fn parse_version(output: &str) -> Option<String> {
    // Expect format: "tool_name X.Y.Z" or just "X.Y.Z"
    let first_line = output.lines().next()?;
    let version_part = first_line.split_whitespace().last()?;
    // Basic semver-ish check
    if version_part.contains('.') {
        Some(version_part.to_string())
    } else {
        Some(first_line.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_with_name() {
        assert_eq!(parse_version("mycelium 0.8.0"), Some("0.8.0".to_string()));
    }

    #[test]
    fn test_parse_version_bare() {
        assert_eq!(parse_version("0.1.0"), Some("0.1.0".to_string()));
    }

    #[test]
    fn test_parse_version_empty() {
        assert_eq!(parse_version(""), None);
    }
}

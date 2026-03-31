use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Mycelium,
    Hyphae,
    Rhizome,
    Cortina,
    Canopy,
    /// Cap is a web dashboard (Node.js, not Rust). Discovered via `cap` binary but
    /// may not be in PATH if run via `npm run dev:all` from a cloned repo.
    Cap,
}

impl Tool {
    #[must_use]
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Mycelium => "mycelium",
            Self::Hyphae => "hyphae",
            Self::Rhizome => "rhizome",
            Self::Cortina => "cortina",
            Self::Canopy => "canopy",
            Self::Cap => "cap",
        }
    }

    #[must_use]
    pub fn all() -> &'static [Tool] {
        &[
            Self::Mycelium,
            Self::Hyphae,
            Self::Rhizome,
            Self::Cortina,
            Self::Canopy,
            Self::Cap,
        ]
    }

    /// Minimum compatible spore version for this tool.
    #[must_use]
    pub fn min_spore_version(self) -> &'static str {
        match self {
            Self::Mycelium
            | Self::Hyphae
            | Self::Rhizome
            | Self::Cortina
            | Self::Canopy
            | Self::Cap => "0.1.0",
        }
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

impl ProjectContext {
    /// Detect project context from a given path.
    ///
    /// Walks up from `path` to find the nearest `.git` directory (project root),
    /// falls back to `path` itself if none found. Detects languages by counting
    /// file extensions in the top 2 directory levels.
    #[must_use]
    pub fn detect(path: &Path) -> Self {
        let root = find_git_root(path).unwrap_or_else(|| path.to_path_buf());

        let name = root.file_name().map_or_else(
            || "unknown".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );

        let detected_languages = detect_languages(&root);

        Self {
            name,
            root,
            detected_languages,
        }
    }
}

/// Walk up from `path` to find the nearest directory containing `.git`.
fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Map a file extension to a language name.
fn ext_to_language(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "hpp" => Some("cpp"),
        "rb" => Some("ruby"),
        _ => None,
    }
}

/// Detect languages by counting file extensions in the top 2 levels of a directory.
/// Returns the top 3 languages by file count.
fn detect_languages(root: &Path) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();

    for depth_0_entry in std::fs::read_dir(root).into_iter().flatten() {
        let Ok(entry) = depth_0_entry else {
            continue;
        };
        let path = entry.path();

        // Level 0: check files at the root
        if path.is_file()
            && let Some(lang) = path
                .extension()
                .and_then(|e| ext_to_language(&e.to_string_lossy()))
        {
            *counts.entry(lang).or_default() += 1;
        }

        // Level 1: check files one directory deeper
        if path.is_dir() {
            for depth_1_entry in std::fs::read_dir(&path).into_iter().flatten() {
                let Ok(child) = depth_1_entry else {
                    continue;
                };
                let child_path = child.path();
                if child_path.is_file()
                    && let Some(lang) = child_path
                        .extension()
                        .and_then(|e| ext_to_language(&e.to_string_lossy()))
                {
                    *counts.entry(lang).or_default() += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));

    ranked
        .into_iter()
        .take(3)
        .map(|(lang, _)| lang.to_owned())
        .collect()
}

/// Detect project context from a given path.
///
/// Convenience free function that delegates to [`ProjectContext::detect`].
#[must_use]
pub fn detect_project(path: &Path) -> ProjectContext {
    ProjectContext::detect(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_detect_finds_git_root() {
        let spore_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let ctx = ProjectContext::detect(&spore_src);

        // The spore project root should contain a .git directory
        assert!(
            ctx.root.join(".git").exists(),
            "Expected root {} to contain .git",
            ctx.root.display()
        );
    }

    #[test]
    fn test_detect_extracts_project_name() {
        let spore_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = ProjectContext::detect(spore_dir);

        let expected_name = spore_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(ctx.name, expected_name);
    }

    #[test]
    fn test_detect_languages() {
        let spore_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let ctx = ProjectContext::detect(spore_dir);

        assert!(
            ctx.detected_languages.contains(&"rust".to_owned()),
            "Expected 'rust' in detected languages: {:?}",
            ctx.detected_languages
        );
    }

    #[test]
    fn test_find_git_root_none_at_filesystem_root() {
        // Root of filesystem shouldn't have .git (in normal environments)
        let result = find_git_root(Path::new("/"));
        // This may or may not be None depending on environment, but shouldn't panic
        drop(result);
    }

    #[test]
    fn test_ext_to_language_mapping() {
        assert_eq!(ext_to_language("rs"), Some("rust"));
        assert_eq!(ext_to_language("py"), Some("python"));
        assert_eq!(ext_to_language("ts"), Some("typescript"));
        assert_eq!(ext_to_language("tsx"), Some("typescript"));
        assert_eq!(ext_to_language("js"), Some("javascript"));
        assert_eq!(ext_to_language("go"), Some("go"));
        assert_eq!(ext_to_language("java"), Some("java"));
        assert_eq!(ext_to_language("c"), Some("c"));
        assert_eq!(ext_to_language("h"), Some("c"));
        assert_eq!(ext_to_language("cpp"), Some("cpp"));
        assert_eq!(ext_to_language("rb"), Some("ruby"));
        assert_eq!(ext_to_language("txt"), None);
    }
}

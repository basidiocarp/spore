//! Platform-aware path resolution for ecosystem tools.
//!
//! Provides consistent config, data, and database path resolution across
//! macOS, Linux, and Windows. All ecosystem tools should use these functions
//! instead of rolling their own path logic.

use std::path::PathBuf;

use anyhow::{Context, Result};

// ─────────────────────────────────────────────────────────────────────────────
// Config Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the config directory for an ecosystem tool.
///
/// Returns `~/.config/<app_name>/` on Linux/macOS, or the platform equivalent
/// via `dirs::config_dir()`.
///
/// # Examples
///
/// ```
/// let dir = spore::paths::config_dir("mycelium");
/// // On macOS: ~/Library/Application Support/mycelium/
/// // On Linux: ~/.config/mycelium/
/// ```
#[must_use]
pub fn config_dir(app_name: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(app_name)
}

/// Resolve the config file path for an ecosystem tool.
///
/// Returns `<config_dir>/<app_name>/config.toml`.
#[must_use]
pub fn config_path(app_name: &str) -> PathBuf {
    config_dir(app_name).join("config.toml")
}

/// Resolve a config file path with an environment variable override.
///
/// Priority:
/// 1. `$<env_var>` environment variable (if set)
/// 2. Platform-specific config dir: `<config_dir>/<app_name>/config.toml`
#[must_use]
pub fn config_path_with_env(app_name: &str, env_var: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env_var) {
        return PathBuf::from(p);
    }
    config_path(app_name)
}

// ─────────────────────────────────────────────────────────────────────────────
// Data Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the data directory for an ecosystem tool.
///
/// Returns `~/.local/share/<app_name>/` on Linux, or the platform equivalent
/// via `dirs::data_local_dir()`.
#[must_use]
pub fn data_dir(app_name: &str) -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(app_name)
}

/// Resolve a database file path for an ecosystem tool.
///
/// Priority:
/// 1. `$<env_var>` environment variable (if set)
/// 2. `override_path` argument (for CLI `--db` flags)
/// 3. Platform-specific data dir: `<data_dir>/<app_name>/<db_filename>`
///
/// Creates the parent directory if it doesn't exist.
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created.
pub fn db_path(
    app_name: &str,
    db_filename: &str,
    env_var: &str,
    override_path: Option<&str>,
) -> Result<PathBuf> {
    let path = if let Some(p) = override_path {
        PathBuf::from(p)
    } else if let Ok(p) = std::env::var(env_var) {
        PathBuf::from(p)
    } else {
        data_dir(app_name).join(db_filename)
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating data directory {}", parent.display()))?;
    }

    Ok(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Project Root Detection
// ─────────────────────────────────────────────────────────────────────────────

/// Find the nearest project root by walking up from `start` looking for marker files.
///
/// Default markers: `.git`, `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml`.
#[must_use]
pub fn find_project_root(start: &std::path::Path) -> Option<PathBuf> {
    find_project_root_with_markers(
        start,
        &[
            ".git",
            "Cargo.toml",
            "package.json",
            "go.mod",
            "pyproject.toml",
        ],
    )
}

/// Find the nearest project root by walking up from `start` looking for any of the
/// given marker files or directories.
#[must_use]
pub fn find_project_root_with_markers(
    start: &std::path::Path,
    markers: &[&str],
) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        for marker in markers {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_returns_path() {
        let dir = config_dir("mycelium");
        assert!(dir.ends_with("mycelium"));
    }

    #[test]
    fn test_config_path_returns_toml() {
        let path = config_path("hyphae");
        assert!(path.ends_with("config.toml"));
        assert!(path.to_string_lossy().contains("hyphae"));
    }

    #[test]
    fn test_data_dir_returns_path() {
        let dir = data_dir("mycelium");
        assert!(dir.ends_with("mycelium"));
    }

    #[test]
    fn test_db_path_with_override() {
        let path = db_path("test", "test.db", "NONEXISTENT_VAR", Some("/tmp/test.db")).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test.db"));
    }

    #[test]
    fn test_db_path_default() {
        let path = db_path("test-app", "history.db", "NONEXISTENT_VAR_12345", None).unwrap();
        assert!(path.to_string_lossy().contains("test-app"));
        assert!(path.ends_with("history.db"));
    }

    #[test]
    fn test_find_project_root_from_spore_src() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let root = find_project_root(&src);
        assert!(root.is_some());
        let root = root.unwrap();
        assert!(root.join("Cargo.toml").exists() || root.join(".git").exists());
    }

    #[test]
    fn test_find_project_root_none_at_filesystem_root() {
        // Shouldn't panic even at /
        let _ = find_project_root(std::path::Path::new("/"));
    }

    #[test]
    fn test_find_project_root_with_custom_markers() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = find_project_root_with_markers(dir, &["Cargo.toml"]);
        assert!(root.is_some());
        assert!(root.unwrap().join("Cargo.toml").exists());
    }
}

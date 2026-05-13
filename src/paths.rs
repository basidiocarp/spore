//! Platform-aware path resolution for ecosystem tools.
//!
//! Provides consistent config, data, and database path resolution across
//! macOS, Linux, and Windows. All ecosystem tools should use these functions
//! instead of rolling their own path logic.

use std::path::PathBuf;

use crate::error::{Result, SporeError};

// ─────────────────────────────────────────────────────────────────────────────
// Config Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the config directory for an ecosystem tool.
///
/// Returns `~/.config/<app_name>/` on Linux/macOS, or the platform equivalent
/// via `dirs::config_dir()`.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when the platform config directory cannot be
/// determined (typically because `HOME` is unset, as in some container or CI
/// environments). Callers in other repos that currently use the previous
/// infallible signature will need updating — see the migration note below.
///
/// # Migration
///
/// **Breaking change from the previous `#[must_use] pub fn config_dir(...) -> PathBuf` API.**
/// The old signature silently fell back to `"."` when `HOME` was unset, producing
/// state files in whatever the current working directory happened to be. The new
/// signature surfaces the error so callers can respond explicitly.
///
/// Consumer repos that call `spore::paths::config_dir` must be updated to handle
/// the `Result`. A typical migration is:
/// ```rust,ignore
/// // Before
/// let dir = spore::paths::config_dir("myapp");
/// // After
/// let dir = spore::paths::config_dir("myapp")?;
/// ```
///
/// # Examples
///
/// ```
/// let dir = spore::paths::config_dir("mycelium");
/// // On macOS: Ok(~/Library/Application Support/mycelium/)
/// // On Linux: Ok(~/.config/mycelium/)
/// ```
pub fn config_dir(app_name: &str) -> Result<PathBuf> {
    dirs::config_dir()
        .ok_or_else(|| {
            SporeError::Path("cannot determine config directory: HOME not set".to_string())
        })
        .map(|base| base.join(app_name))
}

/// Resolve the config file path for an ecosystem tool.
///
/// Returns `<config_dir>/<app_name>/config.toml`.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when the config directory cannot be determined.
pub fn config_path(app_name: &str) -> Result<PathBuf> {
    config_dir(app_name).map(|d| d.join("config.toml"))
}

/// Resolve a config file path with an environment variable override.
///
/// Priority:
/// 1. `$<env_var>` environment variable (if set)
/// 2. Platform-specific config dir: `<config_dir>/<app_name>/config.toml`
///
/// Tilde (`~`) at the start of the env-var value is expanded to the home
/// directory, matching the expansion applied to the config-file path.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when no env-var override is set and the
/// platform config directory cannot be determined.
pub fn config_path_with_env(app_name: &str, env_var: &str) -> Result<PathBuf> {
    if let Ok(p) = std::env::var(env_var) {
        return Ok(expand_tilde(p));
    }
    config_path(app_name)
}

/// Expand a leading `~/` or bare `~` in a path string to the home directory.
///
/// If the home directory cannot be determined the path is returned unchanged.
fn expand_tilde(p: String) -> PathBuf {
    if p == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

// ─────────────────────────────────────────────────────────────────────────────
// Data Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the data directory for an ecosystem tool.
///
/// Returns `~/.local/share/<app_name>/` on Linux, or the platform equivalent
/// via `dirs::data_local_dir()`.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when the platform data directory cannot be
/// determined (typically because `HOME` is unset).
///
/// # Migration
///
/// **Breaking change from the previous `#[must_use] pub fn data_dir(...) -> PathBuf` API.**
/// The old signature silently fell back to `"."`. Consumer repos must update to
/// handle the `Result`.
pub fn data_dir(app_name: &str) -> Result<PathBuf> {
    dirs::data_local_dir()
        .ok_or_else(|| {
            SporeError::Path("cannot determine data directory: HOME not set".to_string())
        })
        .map(|base| base.join(app_name))
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
        data_dir(app_name)?.join(db_filename)
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            SporeError::Path(format!(
                "failed to create data directory {}",
                parent.display()
            ))
        })?;
    }

    Ok(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Capability Registry Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the path for the installed ecosystem capability registry file.
///
/// Default: `<data_dir("basidiocarp")>/capability-registry.json`
///
/// Stipe writes this file after successful install or update. Spore reads it
/// when resolving a capability id to a binary or endpoint candidate.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when the data directory cannot be determined.
pub fn capability_registry_path() -> Result<PathBuf> {
    data_dir("basidiocarp").map(|d| d.join("capability-registry.json"))
}

/// Resolve the directory where runtime capability lease files are stored.
///
/// Default: `<data_dir("basidiocarp")>/leases/`
///
/// Running tools write individual `<capability-id>.json` lease files here when
/// they start serving a capability. Spore reads these when resolving a
/// capability id to a live endpoint.
///
/// # Errors
///
/// Returns [`SporeError::Path`] when the data directory cannot be determined.
pub fn capability_lease_dir() -> Result<PathBuf> {
    data_dir("basidiocarp").map(|d| d.join("leases"))
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
        // HOME is set in a normal dev environment; skip if not.
        if let Ok(dir) = config_dir("mycelium") {
            assert!(dir.ends_with("mycelium"));
        }
    }

    #[test]
    fn test_config_path_returns_toml() {
        if let Ok(path) = config_path("hyphae") {
            assert!(path.ends_with("config.toml"));
            assert!(path.to_string_lossy().contains("hyphae"));
        }
    }

    #[test]
    fn test_data_dir_returns_path() {
        if let Ok(dir) = data_dir("mycelium") {
            assert!(dir.ends_with("mycelium"));
        }
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

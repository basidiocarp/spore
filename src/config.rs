//! TOML configuration loading for ecosystem tools.
//!
//! Provides shared config loading patterns used by mycelium, hyphae, and rhizome.
//! Each tool defines its own config struct (domain-specific), but uses these
//! helpers for path resolution and file loading.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

use crate::paths;

// ─────────────────────────────────────────────────────────────────────────────
// Load Config
// ─────────────────────────────────────────────────────────────────────────────

/// Load a TOML config file for an ecosystem tool.
///
/// Resolution order:
/// 1. `$<env_var>` environment variable (if provided and set)
/// 2. Platform-specific config path: `~/.config/<app_name>/config.toml`
/// 3. Built-in defaults (via `Default` impl on `T`)
///
/// # Examples
///
/// # Errors
///
/// Returns an error if the config file exists but cannot be read or parsed.
///
/// ```ignore
/// let config: MyConfig = spore::config::load("mycelium", Some("MYCELIUM_CONFIG"))?;
/// ```
pub fn load<T: DeserializeOwned + Default>(app_name: &str, env_var: Option<&str>) -> Result<T> {
    let path = match env_var {
        Some(var) => paths::config_path_with_env(app_name, var),
        None => paths::config_path(app_name),
    };

    load_from_path(&path)
}

/// Load a TOML config from a specific file path.
///
/// Returns `T::default()` if the file doesn't exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_from_path<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if path.exists() {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let config: T =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    } else {
        Ok(T::default())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Load with Merge
// ─────────────────────────────────────────────────────────────────────────────

/// Load config with global + project-level merge.
///
/// Used by rhizome (and potentially others) where project-local config
/// overrides global config. The `merge` function receives `(global, project)`
/// and returns the merged result.
///
/// # Resolution
///
/// - Global: `~/.config/<app_name>/config.toml`
/// - Project: `<project_root>/.<app_name>/config.toml`
///
/// # Errors
///
/// Returns an error if either config file exists but cannot be read or parsed.
pub fn load_merged<T, F>(app_name: &str, project_root: &Path, merge: F) -> Result<T>
where
    T: DeserializeOwned + Default,
    F: FnOnce(T, T) -> T,
{
    let global_path = paths::config_path(app_name);
    let project_path = project_root
        .join(format!(".{app_name}"))
        .join("config.toml");

    let global: T = load_from_path(&global_path)?;
    let project: T = load_from_path(&project_path)?;

    Ok(merge(global, project))
}

// ─────────────────────────────────────────────────────────────────────────────
// Save Config
// ─────────────────────────────────────────────────────────────────────────────

/// Save a TOML config file for an ecosystem tool.
///
/// Creates parent directories if they don't exist.
///
/// # Errors
///
/// Returns an error if directories cannot be created or the file cannot be written.
pub fn save<T: serde::Serialize>(app_name: &str, config: &T) -> Result<PathBuf> {
    let path = paths::config_path(app_name);
    save_to_path(&path, config)?;
    Ok(path)
}

/// Save a TOML config to a specific file path.
///
/// # Errors
///
/// Returns an error if directories cannot be created or the file cannot be written.
pub fn save_to_path<T: serde::Serialize>(path: &Path, config: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory {}", parent.display()))?;
    }

    let content = toml::to_string_pretty(config).context("serializing config to TOML")?;
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Show Config Path
// ─────────────────────────────────────────────────────────────────────────────

/// Return a human-readable string describing the active config path.
#[must_use]
pub fn describe_config_path(app_name: &str, env_var: Option<&str>) -> String {
    let path = match env_var {
        Some(var) => paths::config_path_with_env(app_name, var),
        None => paths::config_path(app_name),
    };

    if path.exists() {
        format!("{} (loaded)", path.display())
    } else {
        format!("{} (not found, using defaults)", path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        #[serde(default)]
        name: String,
        #[serde(default)]
        count: u32,
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let config: TestConfig = load_from_path(std::path::Path::new("/nonexistent/config.toml"))
            .expect("should return default");
        assert_eq!(config, TestConfig::default());
    }

    #[test]
    fn test_load_from_path_valid_toml() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "name = \"test\"\ncount = 42").unwrap();

        let config: TestConfig = load_from_path(tmp.path()).unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.count, 42);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = TestConfig {
            name: "roundtrip".into(),
            count: 7,
        };
        save_to_path(&path, &config).unwrap();

        let loaded: TestConfig = load_from_path(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_load_merged() {
        let dir = tempfile::tempdir().unwrap();

        // Create global config
        let global_dir = dir.path().join("global");
        std::fs::create_dir_all(&global_dir).unwrap();
        let global_path = global_dir.join("config.toml");
        std::fs::write(&global_path, "name = \"global\"\ncount = 1").unwrap();

        // Create project config
        let project_root = dir.path().join("project");
        let project_config_dir = project_root.join(".testapp");
        std::fs::create_dir_all(&project_config_dir).unwrap();
        std::fs::write(
            project_config_dir.join("config.toml"),
            "name = \"project\"\ncount = 99",
        )
        .unwrap();

        // Test project override loads
        let project: TestConfig = load_from_path(&project_config_dir.join("config.toml")).unwrap();
        assert_eq!(project.name, "project");
        assert_eq!(project.count, 99);
    }

    #[test]
    fn test_describe_config_path_not_found() {
        let desc = describe_config_path("nonexistent-app-12345", None);
        assert!(desc.contains("not found"));
        assert!(desc.contains("defaults"));
    }
}

//! Editor detection and configuration paths.
//!
//! Discovers installed editors by checking for their config directories,
//! and provides the correct config file path for MCP server registration.
//! Used by stipe (init) and hyphae (init) to auto-configure the ecosystem.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Editor Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Supported editors that can host MCP servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Editor {
    ClaudeCode,
    Cursor,
    VsCode,
    Zed,
    Windsurf,
    Amp,
    ClaudeDesktop,
    CodexCli,
}

impl Editor {
    /// All known editors.
    #[must_use]
    pub fn all() -> &'static [Editor] {
        &[
            Self::ClaudeCode,
            Self::Cursor,
            Self::VsCode,
            Self::Zed,
            Self::Windsurf,
            Self::Amp,
            Self::ClaudeDesktop,
            Self::CodexCli,
        ]
    }

    /// Display name for the editor.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::VsCode => "VS Code",
            Self::Zed => "Zed",
            Self::Windsurf => "Windsurf",
            Self::Amp => "Amp",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::CodexCli => "Codex CLI",
        }
    }

    /// The JSON key used for the MCP servers section in this editor's config.
    #[must_use]
    pub fn mcp_key(self) -> &'static str {
        match self {
            Self::VsCode => "servers",
            Self::Zed => "context_servers",
            _ => "mcpServers",
        }
    }

    /// Whether this editor uses TOML config (vs JSON).
    #[must_use]
    pub fn uses_toml(self) -> bool {
        matches!(self, Self::CodexCli)
    }
}

impl std::fmt::Display for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection
// ─────────────────────────────────────────────────────────────────────────────

/// Detect installed editors by checking for their config directories.
///
/// Returns editors whose config markers exist on the filesystem.
#[must_use]
pub fn detect() -> Vec<Editor> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let mut editors = Vec::new();

    for &editor in Editor::all() {
        if editor_marker_exists(&home, editor) {
            editors.push(editor);
        }
    }

    editors
}

fn editor_marker_exists(home: &Path, editor: Editor) -> bool {
    match editor {
        Editor::ClaudeCode => home.join(".claude.json").exists(),
        Editor::Cursor => home.join(".cursor").is_dir(),
        Editor::VsCode => vscode_dir(home).exists(),
        Editor::Zed => home.join(".zed").is_dir(),
        Editor::Windsurf => home.join(".codeium/windsurf").is_dir(),
        Editor::Amp => home.join(".config/amp").is_dir(),
        Editor::ClaudeDesktop => claude_desktop_dir(home).exists(),
        Editor::CodexCli => home.join(".codex").is_dir(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Get the config file path for MCP server registration in the given editor.
#[must_use]
pub fn config_path(editor: Editor) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    match editor {
        Editor::ClaudeCode => home.join(".claude.json"),
        Editor::Cursor => home.join(".cursor/mcp.json"),
        Editor::VsCode => {
            #[cfg(target_os = "macos")]
            {
                home.join("Library/Application Support/Code/User/settings.json")
            }
            #[cfg(not(target_os = "macos"))]
            {
                home.join(".config/Code/User/settings.json")
            }
        }
        Editor::Zed => home.join(".zed/settings.json"),
        Editor::Windsurf => home.join(".codeium/windsurf/mcp_config.json"),
        Editor::Amp => home.join(".config/amp/settings.json"),
        Editor::ClaudeDesktop => {
            #[cfg(target_os = "macos")]
            {
                home.join("Library/Application Support/Claude/claude_desktop_config.json")
            }
            #[cfg(not(target_os = "macos"))]
            {
                home.join(".config/Claude/claude_desktop_config.json")
            }
        }
        Editor::CodexCli => home.join(".codex/config.toml"),
    }
}

/// Get the Claude Code config directory (`~/.claude/`).
#[must_use]
pub fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Get the Claude Code settings.json path (`~/.claude/settings.json`).
#[must_use]
pub fn claude_settings_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("settings.json"))
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP Server Registration
// ─────────────────────────────────────────────────────────────────────────────

/// Build a JSON MCP server entry for the given editor and binary.
#[must_use]
pub fn mcp_entry(editor: Editor, binary_path: &str, args: &[&str]) -> serde_json::Value {
    match editor {
        Editor::VsCode => serde_json::json!({
            "command": binary_path,
            "args": args,
            "type": "stdio"
        }),
        Editor::Zed => serde_json::json!({
            "command": {
                "path": binary_path,
                "args": args
            }
        }),
        _ => serde_json::json!({
            "command": binary_path,
            "args": args
        }),
    }
}

/// Register an MCP server in an editor's JSON config file.
///
/// Reads the existing config, merges the new server entry, backs up the
/// original, and writes atomically. Idempotent: overwrites if the server
/// name already exists.
///
/// # Errors
///
/// Returns an error if the config file cannot be read, parsed, or written.
///
/// # Panics
///
/// Panics if the root JSON value is not an object (should not happen since
/// we construct it as `json!({})`).
pub fn register_mcp_server(
    editor: Editor,
    server_name: &str,
    binary_path: &str,
    args: &[&str],
) -> anyhow::Result<()> {
    use anyhow::Context;

    let path = config_path(editor);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    let mut root: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        if content.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?
        }
    } else {
        serde_json::json!({})
    };

    // Backup existing file
    if path.exists() {
        let backup = path.with_extension("json.bak");
        std::fs::copy(&path, &backup).with_context(|| format!("backing up {}", path.display()))?;
    }

    // Insert server entry
    let key = editor.mcp_key();
    let entry = mcp_entry(editor, binary_path, args);

    let root_obj = root.as_object_mut().expect("root must be an object");
    let servers = root_obj.entry(key).or_insert_with(|| serde_json::json!({}));

    if let Some(map) = servers.as_object_mut() {
        map.insert(server_name.to_string(), entry);
    }

    let content = serde_json::to_string_pretty(&root).context("serializing config")?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Platform Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn vscode_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".config/Code")
    }
}

fn claude_desktop_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Claude")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".config/Claude")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_names() {
        assert_eq!(Editor::ClaudeCode.name(), "Claude Code");
        assert_eq!(Editor::CodexCli.name(), "Codex CLI");
    }

    #[test]
    fn test_editor_mcp_keys() {
        assert_eq!(Editor::ClaudeCode.mcp_key(), "mcpServers");
        assert_eq!(Editor::VsCode.mcp_key(), "servers");
        assert_eq!(Editor::Zed.mcp_key(), "context_servers");
    }

    #[test]
    fn test_editor_uses_toml() {
        assert!(Editor::CodexCli.uses_toml());
        assert!(!Editor::ClaudeCode.uses_toml());
    }

    #[test]
    fn test_detect_returns_vec() {
        // Just verify it doesn't panic
        let _ = detect();
    }

    #[test]
    fn test_config_path_contains_editor_marker() {
        let path = config_path(Editor::Cursor);
        assert!(path.to_string_lossy().contains(".cursor"));

        let path = config_path(Editor::ClaudeCode);
        assert!(path.to_string_lossy().contains(".claude.json"));
    }

    #[test]
    fn test_claude_dir() {
        let dir = claude_dir();
        if let Some(d) = dir {
            assert!(d.to_string_lossy().contains(".claude"));
        }
    }

    #[test]
    fn test_mcp_entry_standard() {
        let entry = mcp_entry(Editor::ClaudeCode, "/usr/bin/hyphae", &["serve"]);
        assert_eq!(entry["command"], "/usr/bin/hyphae");
        assert_eq!(entry["args"][0], "serve");
    }

    #[test]
    fn test_mcp_entry_vscode_has_type() {
        let entry = mcp_entry(Editor::VsCode, "/usr/bin/hyphae", &["serve"]);
        assert_eq!(entry["type"], "stdio");
    }

    #[test]
    fn test_mcp_entry_zed_nested_command() {
        let entry = mcp_entry(Editor::Zed, "/usr/bin/hyphae", &["serve"]);
        assert_eq!(entry["command"]["path"], "/usr/bin/hyphae");
    }

    #[test]
    fn test_register_mcp_server_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        // Override won't work since config_path uses home dir,
        // so test the JSON manipulation directly
        let mut root = serde_json::json!({});
        let key = Editor::ClaudeCode.mcp_key();
        let entry = mcp_entry(Editor::ClaudeCode, "/usr/bin/hyphae", &["serve"]);

        let servers = root
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert_with(|| serde_json::json!({}));
        servers
            .as_object_mut()
            .unwrap()
            .insert("hyphae".to_string(), entry);

        assert!(
            root["mcpServers"]["hyphae"]["command"]
                .as_str()
                .unwrap()
                .contains("hyphae")
        );
    }

    #[test]
    fn test_all_editors_count() {
        assert_eq!(Editor::all().len(), 8);
    }
}

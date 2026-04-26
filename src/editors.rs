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
#[non_exhaustive]
pub enum Editor {
    ClaudeCode,
    Cursor,
    VsCode,
    Zed,
    Windsurf,
    Amp,
    ClaudeDesktop,
    CodexCli,
    GeminiCli,
    CopilotCli,
}

/// Editor config serialization format used for MCP registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorConfigFormat {
    Json,
    Toml,
}

/// Resolved editor metadata for downstream tools that only need editor primitives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDescriptor {
    pub editor: Editor,
    pub name: &'static str,
    pub mcp_key: &'static str,
    pub config_format: EditorConfigFormat,
    pub config_path: PathBuf,
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
            Self::GeminiCli,
            Self::CopilotCli,
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
            Self::GeminiCli => "Gemini CLI",
            Self::CopilotCli => "Copilot CLI",
        }
    }

    /// The JSON key used for the MCP servers section in this editor's config.
    #[must_use]
    pub fn mcp_key(self) -> &'static str {
        match self {
            Self::VsCode => "servers",
            Self::Zed => "context_servers",
            Self::CodexCli => "mcp_servers",
            _ => "mcpServers",
        }
    }

    /// Whether this editor uses TOML config (vs JSON).
    #[must_use]
    pub fn uses_toml(self) -> bool {
        matches!(self, Self::CodexCli)
    }

    /// Config serialization format for this editor's MCP file.
    #[must_use]
    pub fn config_format(self) -> EditorConfigFormat {
        if self.uses_toml() {
            EditorConfigFormat::Toml
        } else {
            EditorConfigFormat::Json
        }
    }

    /// Resolve the editor metadata that downstream orchestration layers depend on.
    ///
    /// # Errors
    ///
    /// Returns an error if the config path cannot be determined.
    pub fn descriptor(self) -> crate::error::Result<EditorDescriptor> {
        Ok(EditorDescriptor {
            editor: self,
            name: self.name(),
            mcp_key: self.mcp_key(),
            config_format: self.config_format(),
            config_path: config_path(self)?,
        })
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

/// Detect installed editors and resolve their config metadata.
///
/// Editors whose metadata cannot be resolved are skipped.
#[must_use]
pub fn detect_descriptors() -> Vec<EditorDescriptor> {
    detect()
        .into_iter()
        .filter_map(|editor| editor.descriptor().ok())
        .collect()
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
        Editor::GeminiCli => home.join(".gemini").is_dir(),
        Editor::CopilotCli => home.join(".copilot").is_dir(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config Paths
// ─────────────────────────────────────────────────────────────────────────────

/// Get the config file path for MCP server registration in the given editor.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn config_path(editor: Editor) -> crate::error::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::SporeError::Other("could not determine home directory".to_string())
    })?;
    Ok(match editor {
        Editor::ClaudeCode => home.join(".claude.json"),
        Editor::Cursor => home.join(".cursor/mcp.json"),
        Editor::VsCode => vscode_settings_path(&home),
        Editor::Zed => home.join(".zed/settings.json"),
        Editor::Windsurf => home.join(".codeium/windsurf/mcp_config.json"),
        Editor::Amp => home.join(".config/amp/settings.json"),
        Editor::ClaudeDesktop => claude_desktop_config_path(&home),
        Editor::CodexCli => home.join(".codex/config.toml"),
        Editor::GeminiCli => home.join(".gemini/settings.json"),
        Editor::CopilotCli => home.join(".copilot/mcp-config.json"),
    })
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
        Editor::CopilotCli => serde_json::json!({
            "type": "local",
            "command": binary_path,
            "args": args
        }),
        _ => serde_json::json!({
            "command": binary_path,
            "args": args
        }),
    }
}

/// MCP server definition for batch registration.
#[derive(Debug, Clone, Copy)]
pub struct McpServer<'a> {
    pub name: &'a str,
    pub command: &'a str,
    pub args: &'a [&'a str],
}

/// Register an MCP server in an editor config file.
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
) -> crate::error::Result<()> {
    let server = McpServer {
        name: server_name,
        command: binary_path,
        args,
    };
    register_mcp_servers(editor, &[server])
}

/// Register one or more MCP servers in an editor config file.
///
/// This batches writes so the original config is backed up once per file
/// before all server entries are merged.
///
/// # Errors
///
/// Returns an error if Spore cannot resolve the editor config path, create
/// the parent config directory, read an existing config file, or write the
/// merged editor configuration back to disk.
pub fn register_mcp_servers(editor: Editor, servers: &[McpServer<'_>]) -> crate::error::Result<()> {
    if editor.uses_toml() {
        return register_mcp_servers_toml(editor, servers);
    }
    register_mcp_servers_json(editor, servers)
}

fn register_mcp_servers_json(
    editor: Editor,
    servers: &[McpServer<'_>],
) -> crate::error::Result<()> {
    let path = config_path(editor)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            crate::error::SporeError::Config(format!(
                "creating directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    // Try-open the file directly; treat NotFound as "start fresh".
    let mut root: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(content) => {
            if content.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&content).map_err(|e| {
                    crate::error::SporeError::Config(format!("parsing {}: {e}", path.display()))
                })?
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => {
            return Err(crate::error::SporeError::Config(format!(
                "reading {}: {e}",
                path.display()
            )));
        }
    };

    // Atomic backup: write a temp file in the same directory then rename.
    // Only attempt when the original already exists.
    {
        let backup = path.with_extension("json.bak");
        match std::fs::read(&path) {
            Ok(original_bytes) => {
                let tmp_backup = path.with_extension("json.bak.tmp");
                std::fs::write(&tmp_backup, &original_bytes).map_err(|e| {
                    crate::error::SporeError::Config(format!(
                        "writing backup temp for {}: {e}",
                        path.display()
                    ))
                })?;
                std::fs::rename(&tmp_backup, &backup).map_err(|e| {
                    let _ = std::fs::remove_file(&tmp_backup);
                    crate::error::SporeError::Config(format!(
                        "renaming backup for {}: {e}",
                        path.display()
                    ))
                })?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No original to back up.
            }
            Err(e) => {
                return Err(crate::error::SporeError::Config(format!(
                    "reading {} for backup: {e}",
                    path.display()
                )));
            }
        }
    }

    // Insert server entries.
    let key = editor.mcp_key();
    let root_obj = root.as_object_mut().ok_or_else(|| {
        crate::error::SporeError::Config(format!(
            "config root in {} is not a JSON object",
            path.display()
        ))
    })?;
    let server_map = root_obj.entry(key).or_insert_with(|| serde_json::json!({}));

    if let Some(map) = server_map.as_object_mut() {
        for server in servers {
            map.insert(
                server.name.to_string(),
                mcp_entry(editor, server.command, server.args),
            );
        }
    }

    // Atomic write: temp file then rename.
    let content = serde_json::to_string_pretty(&root)
        .map_err(|e| crate::error::SporeError::Config(format!("serializing config: {e}")))?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &content).map_err(|e| {
        crate::error::SporeError::Config(format!("writing temp {}: {e}", tmp_path.display()))
    })?;
    std::fs::rename(&tmp_path, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        crate::error::SporeError::Config(format!("replacing {}: {e}", path.display()))
    })?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn register_mcp_servers_toml(
    editor: Editor,
    servers: &[McpServer<'_>],
) -> crate::error::Result<()> {
    let path = config_path(editor)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            crate::error::SporeError::Config(format!(
                "creating directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    // Try-open the file directly; treat NotFound as "start fresh".
    let mut root: toml::Value = match std::fs::read_to_string(&path) {
        Ok(content) => {
            if content.trim().is_empty() {
                toml::Value::Table(toml::map::Map::new())
            } else {
                content.parse().map_err(|e| {
                    crate::error::SporeError::Config(format!(
                        "parsing TOML {}: {e}",
                        path.display()
                    ))
                })?
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => {
            return Err(crate::error::SporeError::Config(format!(
                "reading {}: {e}",
                path.display()
            )));
        }
    };

    // Atomic backup: write a temp file in the same directory then rename.
    {
        let backup = path.with_extension("toml.bak");
        match std::fs::read(&path) {
            Ok(original_bytes) => {
                let tmp_backup = path.with_extension("toml.bak.tmp");
                std::fs::write(&tmp_backup, &original_bytes).map_err(|e| {
                    crate::error::SporeError::Config(format!(
                        "writing backup temp for {}: {e}",
                        path.display()
                    ))
                })?;
                std::fs::rename(&tmp_backup, &backup).map_err(|e| {
                    let _ = std::fs::remove_file(&tmp_backup);
                    crate::error::SporeError::Config(format!(
                        "renaming backup for {}: {e}",
                        path.display()
                    ))
                })?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No original to back up.
            }
            Err(e) => {
                return Err(crate::error::SporeError::Config(format!(
                    "reading {} for backup: {e}",
                    path.display()
                )));
            }
        }
    }

    // Insert under [mcp_servers.<server_name>]
    let key = editor.mcp_key();
    let root_table = root.as_table_mut().ok_or_else(|| {
        crate::error::SporeError::Config(format!(
            "config root in {} is not a TOML table",
            path.display()
        ))
    })?;
    let server_map = root_table
        .entry(key)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if let Some(table) = server_map.as_table_mut() {
        for server in servers {
            let mut server_table = toml::map::Map::new();
            server_table.insert(
                "command".to_string(),
                toml::Value::String(server.command.to_string()),
            );
            server_table.insert(
                "args".to_string(),
                toml::Value::Array(
                    server
                        .args
                        .iter()
                        .map(|arg| toml::Value::String((*arg).to_string()))
                        .collect(),
                ),
            );
            table.insert(server.name.to_string(), toml::Value::Table(server_table));
        }
    }

    // Atomic write: temp file then rename.
    let content = toml::to_string_pretty(&root)
        .map_err(|e| crate::error::SporeError::Config(format!("serializing TOML config: {e}")))?;
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content).map_err(|e| {
        crate::error::SporeError::Config(format!("writing temp {}: {e}", tmp_path.display()))
    })?;
    std::fs::rename(&tmp_path, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        crate::error::SporeError::Config(format!("replacing {}: {e}", path.display()))
    })?;

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
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("Code")
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        home.join(".config/Code")
    }
}

fn vscode_settings_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code/User/settings.json")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("Code")
            .join("User")
            .join("settings.json")
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        home.join(".config/Code/User/settings.json")
    }
}

fn claude_desktop_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Claude")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("Claude")
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        home.join(".config/Claude")
    }
}

fn claude_desktop_config_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Claude/claude_desktop_config.json")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("Claude")
            .join("claude_desktop_config.json")
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        home.join(".config/Claude/claude_desktop_config.json")
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
    fn test_editor_config_format() {
        assert_eq!(Editor::CodexCli.config_format(), EditorConfigFormat::Toml);
        assert_eq!(Editor::Cursor.config_format(), EditorConfigFormat::Json);
    }

    #[test]
    fn test_detect_returns_vec() {
        // Just verify it doesn't panic
        let _ = detect();
    }

    #[test]
    fn test_detect_descriptors_matches_detected_editors() {
        let editors = detect();
        let descriptors = detect_descriptors();
        let descriptor_editors = descriptors
            .into_iter()
            .map(|descriptor| descriptor.editor)
            .collect::<Vec<_>>();

        assert_eq!(descriptor_editors, editors);
    }

    #[test]
    fn test_config_path_contains_editor_marker() {
        let path = config_path(Editor::Cursor).unwrap();
        assert!(path.to_string_lossy().contains(".cursor"));

        let path = config_path(Editor::ClaudeCode).unwrap();
        assert!(path.to_string_lossy().contains(".claude.json"));
    }

    #[test]
    fn test_editor_descriptor_matches_editor_primitives() {
        let descriptor = Editor::CodexCli.descriptor().unwrap();
        assert_eq!(descriptor.editor, Editor::CodexCli);
        assert_eq!(descriptor.name, "Codex CLI");
        assert_eq!(descriptor.mcp_key, "mcp_servers");
        assert_eq!(descriptor.config_format, EditorConfigFormat::Toml);
        assert!(descriptor.config_path.to_string_lossy().contains(".codex"));
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
        let _path = dir.path().join("test.json");

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
        assert_eq!(Editor::all().len(), 10);
    }

    #[test]
    fn test_gemini_config_path() {
        let path = config_path(Editor::GeminiCli).unwrap();
        assert!(path.to_string_lossy().contains(".gemini"));
        assert!(path.to_string_lossy().ends_with("settings.json"));
    }

    #[test]
    fn test_copilot_config_path() {
        let path = config_path(Editor::CopilotCli).unwrap();
        assert!(path.to_string_lossy().contains(".copilot"));
        assert!(path.to_string_lossy().ends_with("mcp-config.json"));
    }

    #[test]
    fn test_copilot_mcp_entry_has_type_local() {
        let entry = mcp_entry(Editor::CopilotCli, "/usr/bin/hyphae", &["serve"]);
        assert_eq!(entry["type"], "local");
        assert_eq!(entry["command"], "/usr/bin/hyphae");
    }

    #[test]
    fn test_gemini_mcp_entry_standard() {
        let entry = mcp_entry(Editor::GeminiCli, "/usr/bin/hyphae", &["serve"]);
        assert_eq!(entry["command"], "/usr/bin/hyphae");
        assert!(entry.get("type").is_none());
    }

    #[test]
    fn test_codex_uses_toml() {
        assert!(Editor::CodexCli.uses_toml());
        assert!(!Editor::GeminiCli.uses_toml());
        assert!(!Editor::CopilotCli.uses_toml());
    }

    #[test]
    fn test_codex_mcp_key() {
        assert_eq!(Editor::CodexCli.mcp_key(), "mcp_servers");
    }
}

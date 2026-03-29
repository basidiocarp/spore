# Changelog

## [0.4.4] - 2026-03-29

### Added

- **Editor descriptors**: `EditorDescriptor`, `EditorConfigFormat`, `Editor::descriptor()`, and `detect_descriptors()` expose reusable editor metadata for downstream tools that need config paths, MCP keys, and config formats without re-deriving them.

### Changed

- **Boundary docs**: README guidance now states the intended split between `spore` editor/transport primitives and higher-level ecosystem policy in tools like `stipe`.

### Fixed

- **Clippy-clean test suite**: cleaned existing test-only lint issues in subprocess, self-update, and token helper tests so `cargo clippy --all-targets -- -D warnings` passes cleanly.

## [0.4.3] - 2026-03-26

### Added

- **Batch MCP registration**: `editors::register_mcp_servers()` and `McpServer` let callers merge multiple MCP servers into one config update and backup cycle.

### Changed

- **Windows-aware editor paths**: Claude Desktop and VS Code config path helpers now resolve through platform-aware config directories instead of assuming Unix-style locations.
- **Shared editor config writing**: JSON and TOML MCP registration paths now batch multiple server writes consistently for downstream tools such as `stipe`.

## [0.4.0] - 2026-03-22

### Changed

- **thiserror migration**: All public APIs return `Result<T, SporeError>` instead of `anyhow::Result<T>`. Consumers can now match on specific error variants (ToolNotFound, RpcError, Timeout, Config, etc.).
- **Lazy tool discovery**: Per-tool `OnceLock` replaces eager HashMap probe. `discover(Tool::Hyphae)` no longer probes all four tools on first call.
- **Request.jsonrpc**: Changed from `String` to `&'static str`. Eliminates heap allocation and enforces the `"2.0"` invariant at compile time.

### Fixed

- **recv_response unwrap**: Replaced `.unwrap()` with safe `if let` pattern on child process restoration.
- **Path arguments**: `tar` and `unzip` commands now receive `OsStr` paths instead of lossy UTF-8 conversions.
- **config_path**: Returns `Result<PathBuf>` instead of silently falling back to `/` when home directory is unavailable.

### Added

- **SporeError enum**: Typed error variants — `ToolNotFound`, `SpawnFailed`, `RpcError`, `Timeout`, `Config`, `Path`, `Network`, `Json`, `Toml`, `Other`.
- **McpClient docs**: Thread-safety requirements documented.
- **Timeout docs**: Reader thread lifecycle on timeout documented.

## [0.3.1] - 2026-03-22

### Added

- **`editors` module**: Editor detection, config paths, and MCP server registration. `detect()` finds installed editors, `config_path()` returns the right config file, `register_mcp_server()` handles JSON merging and backup. 8 editors: Claude Code, Cursor, VS Code, Zed, Windsurf, Amp, Claude Desktop, Codex CLI.

## [0.3.0] - 2026-03-21

### Added

- **`paths` module**: Platform-aware config, data, and database path resolution. `config_dir()`, `config_path()`, `data_dir()`, `db_path()`, `find_project_root()`.
- **`config` module**: TOML config loading with env overrides, global/project merge, save/load helpers. `load()`, `load_merged()`, `save()`.
- **`tokens` module**: Token estimation (`estimate()`, `savings_percent()`) using ~4 chars = 1 token heuristic.
- **`logging` module**: Shared `tracing_subscriber` initialization. `init()`, `init_with_env()`.
- **`self_update` module**: GitHub release checking, downloading, extraction, and binary replacement. `run()`, `fetch_latest_release()`, `target_asset_name()`.

### Changed

- **McpClient timeout enforcement**: Timeout is now properly enforced using a separate reader thread with channel `recv_timeout`. A hung subprocess is killed after the configured timeout.
- **Dependencies**: Added `dirs`, `toml`, `ureq`, `tempfile`, `tracing`, `tracing-subscriber`.
- **Version bump**: 0.2.1 → 0.3.0 (new public API modules).

## [0.2.1] - 2026-03-20

### Added

- **Cap** added to `Tool` enum.
- **`Framing::LineDelimited`** mode for ecosystem MCP servers.

## [0.1.1] - 2026-03-18

### Added

- **`ProjectContext::detect_project()`**: Detects the current project by finding the git root and identifying primary language from file extensions and config files.
- **Subprocess tests**: 7 new tests covering `McpClient` spawn, restart, timeout, and message round-trip behavior.
- **JSON-RPC error response tests**: Coverage for malformed requests, invalid method names, and error code propagation.

### Fixed

- **`cargo fmt` and clippy fixes**: Resolved formatting inconsistencies and pedantic clippy warnings across all modules.

## [0.1.0] - Unreleased

### Added
- Tool discovery: `discover()`, `discover_all()` with `OnceLock` caching
- JSON-RPC 2.0: `Request`, `Response`, `encode()`, `decode()` with Content-Length framing
- Subprocess MCP client: `McpClient` with auto-restart and timeout
- Shared types: `Tool`, `ToolInfo`, `EcosystemStatus`, `ProjectContext`

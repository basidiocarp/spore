# Changelog

All notable changes to Spore are documented in this file.

## [Unreleased]

### Added

- **Shared logging contract**: `spore::logging` now exposes app-aware init
  helpers, safe `try_init` variants, a typed logging config surface, and
  documented tracing guidance for failure localization.

### Changed

- **Docs cleanup**: The changelog and README were refreshed, and `INTERNALS.md`
  moved under `docs/`.

## [0.4.6] - 2026-03-31

### Added

- **Complete tool registry**: `Tool` and `discover_all()` now include
  `cortina` and `canopy`, so downstream callers can rely on Spore for the full
  first-party tool inventory.

## [0.4.5] - 2026-03-30

### Added

- **Shared error envelope**: `error::EcosystemError` now provides a versioned,
  serializable cross-tool failure shape.
- **Canonical protocol spec**: `PROTOCOL.md` now lives in Spore as the shared
  transport and envelope reference for ecosystem MCP traffic.

### Changed

- **README protocol guidance**: The README now points consumers at the
  canonical protocol spec alongside the crate's shared primitives.

## [0.4.4] - 2026-03-29

### Added

- **Editor descriptors**: `EditorDescriptor`, `EditorConfigFormat`,
  `Editor::descriptor()`, and `detect_descriptors()` now expose reusable editor
  metadata for downstream tools.

### Changed

- **Boundary docs**: README guidance now states the intended split between
  Spore's editor and transport primitives and higher-level policy in tools such
  as Stipe.

### Fixed

- **Clippy-clean test suite**: Test-only lint issues in subprocess, self-update,
  and token helper tests were cleaned up so `cargo clippy --all-targets -- -D
  warnings` passes cleanly.

## [0.4.3] - 2026-03-26

### Added

- **Batch MCP registration**: `editors::register_mcp_servers()` and
  `McpServer` now let callers merge multiple MCP servers in one config update
  and backup cycle.

### Changed

- **Windows-aware editor paths**: Claude Desktop and VS Code config helpers now
  resolve through platform-aware config directories.
- **Shared config writing**: JSON and TOML MCP registration paths now batch
  multiple server writes consistently for downstream tools such as Stipe.

## [0.4.0] - 2026-03-21

### Added

- **Typed error surface**: Added `SporeError` with variants such as
  `ToolNotFound`, `SpawnFailed`, `RpcError`, `Timeout`, `Config`, `Path`, and
  `Other`.
- **MCP client docs**: Documented thread-safety expectations and timeout reader
  lifecycle behavior.

### Changed

- **Public error types**: Public APIs now return `Result<T, SporeError>`
  instead of `anyhow::Result<T>`.
- **Lazy tool discovery**: Per-tool `OnceLock` replaced the eager HashMap
  probe.
- **Static JSON-RPC version**: `Request.jsonrpc` now uses `&'static str`
  instead of `String`.

### Fixed

- **Response restoration**: Child-process restoration now uses a safe `if let`
  pattern instead of `.unwrap()`.
- **Path argument handling**: `tar` and `unzip` now receive `OsStr` paths
  instead of lossy UTF-8 conversions.
- **Config path errors**: `config_path()` now returns `Result<PathBuf>` instead
  of silently falling back to `/`.

## [0.3.1] - 2026-03-22

### Added

- **Editors module**: Added editor detection, config-path resolution, and MCP
  server registration for Claude Code, Cursor, VS Code, Zed, Windsurf, Amp,
  Claude Desktop, and Codex CLI.

## [0.3.0] - 2026-03-21

### Added

- **Paths module**: Added platform-aware config, data, database, and project
  root resolution helpers.
- **Config module**: Added TOML config loading with env overrides and
  global-project merge helpers.
- **Tokens module**: Added token-estimation helpers with the approximate four
  chars per token heuristic.
- **Logging module**: Added shared `tracing_subscriber` initialization.
- **Self-update module**: Added GitHub release checking, download, extraction,
  and binary replacement helpers.

### Changed

- **MCP timeout enforcement**: Hung subprocesses are now killed after the
  configured timeout instead of waiting indefinitely.
- **Dependency surface**: Added `dirs`, `toml`, `ureq`, `tempfile`, `tracing`,
  and `tracing-subscriber` with the new public modules.

## [0.2.1] - 2026-03-20

### Added

- **Cap discovery**: Added Cap to the `Tool` enum.
- **Line-delimited framing**: Added `Framing::LineDelimited` mode for ecosystem
  MCP servers.

## [0.1.1] - 2026-03-18

### Added

- **Project detection**: Added `ProjectContext::detect_project()` for git-root
  and primary-language detection.
- **Subprocess coverage**: Added tests for `McpClient` spawn, restart, timeout,
  and message roundtrip behavior.
- **JSON-RPC error coverage**: Added tests for malformed requests, invalid
  method names, and error-code propagation.

### Fixed

- **Formatting and lint fixes**: Cleaned formatting inconsistencies and pedantic
  clippy issues across the crate.

## [0.1.0] - 2026-03-16

### Added

- **Tool discovery**: Added `discover()` and `discover_all()` with `OnceLock`
  caching.
- **JSON-RPC types**: Added `Request`, `Response`, `encode()`, and `decode()`
  with Content-Length framing.
- **Subprocess MCP client**: Added `McpClient` with auto-restart and timeout
  handling.
- **Shared ecosystem types**: Added `Tool`, `ToolInfo`, `EcosystemStatus`, and
  `ProjectContext`.

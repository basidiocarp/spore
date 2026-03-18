# Changelog

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

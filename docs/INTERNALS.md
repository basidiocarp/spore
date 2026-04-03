# Spore Internals

Pure library crate with four modules providing tool discovery, JSON-RPC 2.0 primitives, and subprocess MCP communication.

## Module Layout

```
src/
├── lib.rs           # Public API re-exports
├── discovery.rs     # Tool discovery and caching
├── jsonrpc.rs       # JSON-RPC 2.0 encoding/decoding
├── subprocess.rs    # McpClient: subprocess lifecycle and framing
└── types.rs         # Tool enum, ToolInfo, ProjectContext detection
```

## Discovery: Binary Probing and Caching

`discovery.rs` provides two entry points:

- `discover(tool: Tool) -> Option<ToolInfo>`—find a specific tool
- `discover_all() -> Vec<ToolInfo>`—find all ecosystem tools

Caching uses `OnceLock<HashMap<Tool, Option<ToolInfo>>>` initialized on first call. All subsequent calls hit the cache (zero-cost after first discovery). This is safe because tool paths don't change during process lifetime.

Probing: for each tool, `probe()` calls `which::which()` to locate the binary, then runs `<binary> --version` and parses the output. Version parsing is lenient: splits on whitespace, takes the last token if it contains dots (semver), otherwise falls back to first line. Failures are cached as `None` and won't retry.

## JSON-RPC 2.0: Encoding and Decoding

`jsonrpc.rs` defines four types:

- `Request`—method + params, assigns auto-incrementing `id`
- `Response`—result or error, mirrors request `id`
- `RpcError`—error code + message + optional data
- `encode(request) -> String`—produces `Content-Length: N\r\n\r\n{json}`
- `decode(input: &str) -> Result<Response>`—strips headers, parses body

Encode always uses Content-Length headers (LSP-style). Decode accepts three formats:
- `Content-Length: N\r\n\r\n<body>` (standard LSP)
- `Content-Length: N\n\n<body>` (Unix line endings)
- Bare JSON (fallback, no headers)

ID generation uses an atomic counter `NEXT_ID` that starts at 1 and increments for each request, allowing concurrent `Request::new()` calls from multiple threads.

## McpClient: Subprocess Lifecycle and Framing

`subprocess.rs` manages a single long-lived subprocess with the MCP server.

Lifecycle:
1. `spawn(tool, args)`—discovers binary, spawns process, pipes stdin/stdout
2. `call_tool(name, arguments)`—sends request, waits for response, handles errors
3. Auto-restart on dead process via `ensure_alive()`—kills old child, respawns if needed
4. `Drop` impl kills child on cleanup

Framing modes:

- `LineDelimited` (default)—write JSON as `{...}\n`, read response as `{...}\n`
- `ContentLength`—write/read with LSP-style `Content-Length: N\r\n\r\n{body}`

Request encoding delegates to `jsonrpc::encode()`. Response decoding uses mode-specific readers:
- `read_line_delimited()`—single `reader.read_line()`, parse as JSON
- `read_content_length()`—parse headers in loop until blank line, read exact body bytes

Both framing modes fail fast on EOF, malformed headers, or parse errors. Returns `anyhow::Result` with context.

Subprocess I/O uses `BufReader` for stdout, `std::process::Stdio` for piped stdin/stdout, and redirects stderr to `/dev/null`.

## Types: Tool Enum and ProjectContext Detection

`types.rs` defines:

- `Tool` enum—four ecosystem binaries: `Mycelium`, `Hyphae`, `Rhizome`, `Cap`
- `ToolInfo`—discovered tool with binary path and version
- `EcosystemStatus`—vec of tools + UTC timestamp
- `ProjectContext`—project name, root path, detected languages

Tool methods:
- `binary_name()`—maps enum to binary name (e.g., `Tool::Hyphae` → `"hyphae"`)
- `all()`—returns static slice of all four tools
- `min_spore_version()`—compatibility check (currently all return `"0.1.0"`)

`ProjectContext::detect(path: Path)`:
1. Walk up from path to find nearest `.git` directory (git root)
2. Extract project name from root directory name
3. Scan top 2 directory levels for source files
4. Count by extension, rank by frequency, return top 3 languages

Language mapping supports: Rust, Python, TypeScript/TSX, JavaScript/JSX, Go, Java, C/C++, Ruby. Returns `Vec<String>` of detected language names in descending frequency.

Git root walking converts files to parent, then loops up the tree. Returns `None` if filesystem root is reached without finding `.git`.

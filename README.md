# Spore

Shared IPC primitives for the [Basidiocarp](https://github.com/basidiocarp) ecosystem. Named after fungal spores—lightweight carriers of information between separate organisms.

Spore provides the shared primitives used across Mycelium, Hyphae, Rhizome, and Stipe:

1. Tool discovery—find sibling tools in PATH, cache results, detect versions
2. JSON-RPC 2.0—encode/decode MCP protocol messages with Content-Length framing
3. Subprocess communication—spawn and talk to sibling MCP servers over stdio
4. Editor primitives—detect supported editors, resolve MCP config paths, and write MCP registrations

Spore should stay focused on reusable editor and transport primitives. Ecosystem policy such as install profiles, tool inventory, doctor severity, release mapping, and multi-tool orchestration belongs in higher-level apps like `stipe`.

Protocol note: the canonical transport and envelope specification for ecosystem
MCP traffic lives in [PROTOCOL.md](/Users/williamnewton/projects/claude-mycelium/spore/PROTOCOL.md).

### Detect editors and resolve their MCP metadata

```rust
use spore::editors::{detect_descriptors, Editor, EditorConfigFormat};

let descriptors = detect_descriptors();
for descriptor in descriptors {
    println!(
        "{} writes {:?} MCP config to {}",
        descriptor.name,
        descriptor.config_format,
        descriptor.config_path.display()
    );
}

let codex = Editor::CodexCli.descriptor()?;
assert_eq!(codex.config_format, EditorConfigFormat::Toml);
assert_eq!(codex.mcp_key, "mcp_servers");
```

## Usage

```toml
[dependencies]
spore = { git = "https://github.com/basidiocarp/spore", tag = "v0.1.0" }
```

### Discover sibling tools

```rust
use spore::{discover, Tool};

if let Some(info) = discover(Tool::Hyphae) {
    println!("Found {} v{} at {}", info.tool, info.version, info.binary_path.display());
}

let all = spore::discover_all();
println!("{} ecosystem tools found", all.len());
```

### Spawn an MCP client

```rust
use spore::{McpClient, Tool};
use serde_json::json;

let mut client = McpClient::spawn(Tool::Hyphae, &["serve"])?;

let result = client.call_tool("hyphae_memory_store", json!({
    "content": "Auth module refactored to use JWT",
    "topic": "auth"
}))?;

println!("Stored: {}", result);
```

### JSON-RPC encoding

```rust
use spore::jsonrpc::{Request, encode, decode};

let req = Request::new("tools/call", json!({"name": "get_symbols"}));
let wire = encode(&req);  // Content-Length: N\r\n\r\n{json}

let response = decode(&raw_response)?;
```

## Ecosystem

| Tool | Repo | Purpose |
|------|------|---------|
| [Mycelium](https://github.com/basidiocarp/mycelium) | CLI proxy | Token-optimized command output |
| [Hyphae](https://github.com/basidiocarp/hyphae) | Memory system | Persistent agent memory |
| [Rhizome](https://github.com/basidiocarp/rhizome) | Code intelligence | Symbol extraction and navigation |
| [Cap](https://github.com/basidiocarp/cap) | Dashboard | Web UI for the ecosystem |

## Development

```bash
cargo build
cargo test
cargo clippy
cargo fmt
```

## License

See [LICENSE](LICENSE) for details.

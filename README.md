# Spore

Shared IPC primitives for the [Basidiocarp](https://github.com/basidiocarp) ecosystem. Named after fungal spores — lightweight carriers of information between separate organisms.

Spore provides three capabilities used across Mycelium, Hyphae, and Rhizome:

1. **Tool Discovery** — find sibling tools in PATH, cache results, detect versions
2. **JSON-RPC 2.0** — encode/decode MCP protocol messages with Content-Length framing
3. **Subprocess Communication** — spawn and talk to sibling MCP servers over stdio

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

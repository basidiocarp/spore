# Spore

Shared IPC and editor primitives for the Basidiocarp ecosystem. Provides the
reusable transport, discovery, and config-writing pieces that higher-level
tools build on.

Named after fungal spores, lightweight carriers that move information and
propagation across separate organisms.

Part of the [Basidiocarp ecosystem](https://github.com/basidiocarp).

---

## The Problem

Every ecosystem tool needs the same low-level plumbing: discover sibling
binaries, speak JSON-RPC over stdio, detect editor config paths, and register
MCP servers. Rebuilding that logic in every binary creates drift.

## The Solution

Spore is the shared crate for those primitives. It handles discovery, transport,
subprocess communication, and editor metadata once so the higher-level tools do
not each need their own half-compatible version.

---

## The Ecosystem

| Tool | Purpose |
|------|---------|
| **[spore](https://github.com/basidiocarp/spore)** | Shared transport and editor primitives |
| **[hyphae](https://github.com/basidiocarp/hyphae)** | Persistent agent memory |
| **[mycelium](https://github.com/basidiocarp/mycelium)** | Token-optimized command output |
| **[rhizome](https://github.com/basidiocarp/rhizome)** | Code intelligence via tree-sitter and LSP |
| **[stipe](https://github.com/basidiocarp/stipe)** | Ecosystem installer and manager |

> **Boundary:** `spore` owns reusable transport and editor primitives.
> `stipe` owns ecosystem policy such as install profiles, tool inventory, and
> doctor severity.

---

## Quick Start

```toml
[dependencies]
spore = { git = "https://github.com/basidiocarp/spore", tag = "v0.1.0" }
```

```rust
use spore::{discover, McpClient, Tool};
```

---

## How It Works

```text
App crate               Spore                         Ecosystem tool
─────────               ─────                         ──────────────
discover tool     ─►    tool registry          ─►    PATH lookup
spawn client      ─►    MCP client             ─►    stdio server
write config      ─►    editor descriptors     ─►    host config file
```

1. Discover tools: locate sibling binaries and cache version information.
2. Speak MCP transport: encode and decode JSON-RPC with Content-Length framing.
3. Spawn clients: talk to sibling MCP servers over stdio.
4. Resolve editors: detect supported editors and their config file shapes.

---

## Key Features

- Tool discovery: finds sibling tools in PATH and reports version metadata.
- JSON-RPC transport: provides the shared MCP wire implementation.
- Subprocess clients: spawn and communicate with sibling servers.
- Editor primitives: resolve config paths and format details for supported hosts.

---

## Architecture

```text
spore/
├── src/        shared discovery, JSON-RPC, MCP, and editor modules
├── tests/      integration coverage
└── docs/       protocol and internal notes
```

---

## Documentation

- [PROTOCOL.md](PROTOCOL.md): canonical transport and envelope specification
- [docs/INTERNALS.md](docs/INTERNALS.md): implementation notes and module boundaries

## Development

```bash
cargo build
cargo test
cargo clippy
cargo fmt
```

## License

See [LICENSE](LICENSE) for details.

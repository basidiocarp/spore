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
spore = { git = "https://github.com/basidiocarp/spore", tag = "v0.4.9" }
```

```rust
use spore::{discover, McpClient, Tool};
```

```rust
use tracing::Level;

spore::logging::init_app("hyphae", Level::WARN);
```

```rust
use tracing::Level;

let config = spore::logging::LoggingConfig::for_app("hyphae", Level::WARN);
let context = config.span_context().with_request_id("req-42");
let _root = spore::logging::root_span(&context).entered();
let _request = spore::logging::request_span("mcp_call", &context).entered();
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
- Shared logging: app-aware env vars, safe `try_init` paths, and MCP-safe stderr defaults.
- Default feature split: `logging` and `http` stay on for compatibility, but consumers can disable default features when they want a slimmer embed.

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

- [docs/README.md](docs/README.md): docs index and reading order
- [PROTOCOL.md](PROTOCOL.md): canonical transport and envelope specification
- [docs/internals.md](docs/internals.md): implementation notes and module boundaries

## Logging Contract

`spore::logging` is the shared logging and tracing setup surface for the Rust
ecosystem tools.

- `init_app("rhizome", ...)` derives `RHIZOME_LOG` consistently.
- `try_init...` variants are available for tests, embedded startup, or repeated
  initialization paths where panics are not acceptable.
- `LoggingConfig` provides a small policy surface for output format, output
  target, and span event verbosity.
- `LoggingConfig::span_context()` builds the standard service/session context
  for downstream spans.
- `stderr` remains the default output target for MCP-aware tools so stdout
  transport framing stays clean.
- `root_span`, `request_span`, `tool_span`, `workflow_span`, and
  `subprocess_span` provide one repo-consistent field convention for tracing.

For failure localization, downstream repos should create spans around request,
subprocess, session, and workflow boundaries instead of relying on free-form log
messages alone. Good context fields include `service`, `tool`, `request_id`,
`session_id`, and `workspace_root`. Use the helpers in `spore::logging` so the
field names stay stable across repos instead of hand-rolling new conventions.

## Development

```bash
cargo build
cargo nextest run
cargo test
cargo clippy
cargo fmt
```

- Prefer `cargo nextest run` for the normal test loop.
- Keep `criterion` out of scope here until a concrete hot path is named.
- Because `spore` is a shared library crate, use targeted test timing or
  downstream integration timing instead of a repo-local whole-command run.
- If you are embedding `spore` in a slim consumer, disable default features and
  opt back into `logging` and `http` only when you need those surfaces.
- `spore` keeps `tracing-subscriber` and `ureq` on lean feature sets, and the
  crate uses a `profile.dev` policy that matches the other Rust repos in this
  workspace.

## License

See [LICENSE](LICENSE) for details.

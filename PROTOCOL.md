# Basidiocarp MCP Protocol

Canonical protocol specification for cross-tool MCP communication in the
Basidiocarp ecosystem.

This document is owned by `spore` because `spore` provides the shared JSON-RPC
and subprocess transport primitives used by the Rust tools. Root `contracts/`
schemas define payload shapes for individual cross-tool boundaries; this file
defines the transport and envelope rules those payloads ride on.

## Scope

This specification covers:

- JSON-RPC 2.0 over stdio
- framing conventions
- initialization handshake
- tool-call request and response envelopes
- error taxonomy and code ranges
- naming conventions
- shared identity fields

This specification does not replace the payload schemas in
`/Users/williamnewton/projects/claude-mycelium/contracts/`. Use both:

1. `spore/PROTOCOL.md` for transport and envelope rules
2. `contracts/*.schema.json` for boundary-specific payload shapes

## Transport

- Protocol: JSON-RPC 2.0 over stdio
- Preferred framing: newline-delimited JSON, one JSON object per line
- Alternate framing: `Content-Length` headers only when talking to LSP-style
  servers or another documented non-ecosystem peer
- `stderr` is reserved for logs and diagnostics only. Never write protocol
  messages to `stderr`.

## Framing Rules

### Ecosystem Default

Use newline-delimited JSON for ecosystem MCP servers:

- `hyphae`
- `rhizome`
- any new ecosystem MCP server unless explicitly documented otherwise

Example:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

### Alternate Mode

Use `Content-Length` framing only when the peer requires it, such as LSP
servers:

```text
Content-Length: 84

{"jsonrpc":"2.0","id":1,"method":"textDocument/definition","params":{...}}
```

## Initialization Handshake

Clients should initialize MCP servers with the standard sequence:

1. `initialize`
2. `notifications/initialized`
3. `tools/list` and `tools/call`

Example initialize request:

```json
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "mycelium",
      "version": "0.7.4"
    }
  }
}
```

## Request Envelope

Tool calls use `tools/call` with a tool name and JSON object arguments:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "hyphae_memory_store",
    "arguments": {
      "topic": "decisions",
      "content": "Chose SQLite over Postgres for local storage"
    }
  }
}
```

Rules:

- `id` is required for requests and omitted for notifications
- `method` must be one of the supported MCP methods
- `params.name` is the tool name
- `params.arguments` must match the published schema for that tool boundary

## Response Envelope

### Success

Successful tool execution returns JSON-RPC `result`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"stored\":true,\"id\":\"abc123\"}"
      }
    ]
  }
}
```

### Tool Error

If the tool executed but rejected the request or failed in-domain, return a
tool result with `isError: true` rather than a top-level JSON-RPC protocol
error:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": true,
    "content": [
      {
        "type": "text",
        "text": "Memory with ID 'xyz' not found"
      }
    ]
  }
}
```

### Protocol Error

Malformed requests, unsupported methods, and framing failures return top-level
JSON-RPC `error`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32601,
    "message": "Method not found: tools/invalid"
  }
}
```

## Structured Payload Rule

When a tool returns structured domain data, that structured payload should be
serialized into the first `content[].text` item unless the peer contract
explicitly documents another shape.

Rules:

- prefer a single `content[0].text` item for machine-readable structured output
- payloads inside that text should be JSON objects, not prose
- structured payloads should carry a `schema_version` field when they cross a
  repo boundary
- consumers should reject unknown or missing `schema_version` values when the
  boundary contract requires versioning

## Error Code Allocation

### JSON-RPC Reserved

- `-32700` to `-32600`: JSON-RPC reserved
- `-32099` to `-32000`: MCP or server-reserved transport range

### Ecosystem Tool Ranges

Use these ranges for top-level tool-specific protocol errors when a tool must
surface one:

| Range | Owner |
|-------|-------|
| `1000-1999` | Hyphae |
| `2000-2999` | Rhizome |
| `3000-3999` | Mycelium |
| `4000-4999` | Canopy |
| `5000-5999` | Stipe |
| `6000-6999` | Cortina |

Guidance:

- prefer tool-result `isError` for normal domain failures
- reserve top-level JSON-RPC `error` for protocol, framing, or method-layer
  failure
- if a tool emits a top-level tool-specific code, keep it stable and document
  it in the tool repo

## Naming Conventions

- Use `snake_case`
- Hyphae tools keep the `hyphae_` prefix
- Rhizome expanded-mode tools may remain unprefixed for IDE ergonomics
- Unified wrapper tools should document their command multiplexer explicitly

Examples:

- `hyphae_memory_store`
- `hyphae_import_code_graph`
- `get_symbols`
- `find_references`

## Shared Identity Envelope

Cross-tool boundaries should use the shared identity fields where applicable:

| Field | Type | Meaning |
|-------|------|---------|
| `project` | string | Logical project name |
| `project_root` | string | Canonical repository root path |
| `worktree_id` | string | Worktree identifier |
| `session_id` | string | Runtime session identifier when available |
| `scope` | string | Parallel worker/runtime scope when the boundary supports it |

Rules:

- send both `project_root` and `worktree_id`, or neither
- do not send partial identity pairs
- treat `session_id` as additive, not a substitute for project identity
- if `scope` is supported for a boundary, it participates in identity and must
  not be silently dropped in identity-v1 mode

## Timeouts

Default timeout guidance:

| Caller | Target | Timeout |
|--------|--------|---------|
| `mycelium` | `hyphae` | persistent client, no per-call timeout by default |
| `mycelium` | `rhizome` | 3s |
| `rhizome` | `hyphae` | 10s |
| `cap` | `rhizome` | 10s |
| `cortina` | `hyphae` | fire-and-forget CLI path |

If a caller diverges from these defaults, document the reason in that repo.

## Heartbeat

No ecosystem-wide `ping` method is standardized yet.

Current rule:

- long-lived clients should treat EOF, timeout, or malformed framing as a dead
  peer and reconnect or fail closed

Future work:

- standardize a lightweight `ping` or health method once more tools need
  persistent bidirectional coordination

## Compatibility Rule

- strict internals, tolerant edges
- new repo-to-repo boundaries must adopt the current protocol and published
  schema from the start
- once both sides of a boundary have migrated, remove internal legacy fallback
  paths rather than keeping silent compatibility forever

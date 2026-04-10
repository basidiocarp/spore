# Spore Agent Notes

## Purpose

Spore is the shared Rust infrastructure layer for the ecosystem. Work here should keep the crate library-only, keep primitives reusable across consumers, and keep product policy in the consuming repos. Spore owns shared transport, discovery, config, paths, logging, and related helpers.

---

## Source of Truth

- `src/`: public modules and shared primitives.
- `src/jsonrpc.rs`: JSON-RPC framing and shared request or response types.
- `src/discovery.rs`: sibling-tool discovery behavior.
- `src/paths.rs` and `src/config.rs`: shared path and config resolution.
- `src/logging.rs`: shared tracing setup and span helpers.
- `../ecosystem-versions.toml`: shared dependency pins across consumers.

When a Spore API changes, the crate code is the source of truth and the consumers need coordinated updates.

---

## Before You Start

Before writing code, verify:

1. **Owning primitive**: keep domain semantics in consumers and infrastructure here.
2. **Consumer impact**: identify which repos depend on the API you are changing.
3. **Version pins**: check `../ecosystem-versions.toml` before changing shared dependencies.
4. **Validation target**: decide whether the change needs unit coverage only or follow-up consumer validation too.

---

## Preferred Commands

Use these for most work:

```bash
cargo build
cargo test
```

For targeted work:

```bash
cargo clippy
cargo fmt --check
cargo fmt
```

---

## Repo Architecture

Spore is healthiest when it stays small, reusable, and boring.

Key boundaries:

- `src/jsonrpc.rs`: shared JSON-RPC framing.
- `src/discovery.rs`: sibling-tool discovery.
- `src/config.rs` and `src/paths.rs`: config and path resolution.
- `src/logging.rs`: shared tracing setup and span helpers.
- `src/error.rs` and `src/types.rs`: common infrastructure-facing error and type surfaces.

Current direction:

- Keep shared primitives stable enough that consumers do not drift.
- Keep path, logging, and transport behavior centralized instead of reimplemented downstream.
- Keep Spore free of product-level policy.

---

## Working Rules

- Do not move sibling-tool product semantics into Spore.
- Treat public API changes as ecosystem work, not local refactors.
- Prefer focused unit tests because the crate surface is reusable infrastructure.
- When a change affects discovery, paths, or JSON-RPC framing, think through the consumer impact explicitly.

---

## Multi-Agent Patterns

For substantial Spore work, default to two agents:

**1. Primary implementation worker**
- Owns the touched shared primitive or API surface
- Keeps the write scope inside Spore unless consumer follow-up is required

**2. Independent validator**
- Reviews the broader shape instead of redoing the implementation
- Specifically looks for public API breakage, duplicated downstream policy, path drift, and JSON-RPC framing regressions

Add a docs worker when `README.md`, `CLAUDE.md`, `AGENTS.md`, or public docs changed materially.

---

## Skills to Load

Use these for most work in this repo:

- `basidiocarp-rust-repos`: repo-local Rust workflow and validation habits
- `systematic-debugging`: before fixing unexplained shared-infra regressions
- `writing-voice`: when touching README or docs prose

Use these when the task needs them:

- `test-writing`: when public API behavior changes need stronger coverage
- `basidiocarp-workspace-router`: when the change may spill into consumer repos
- `tool-preferences`: when exploration should stay tight

---

## Done Means

A task is not complete until:

- [ ] The change is in the right shared primitive or API layer
- [ ] The narrowest relevant validation has run, when practical
- [ ] Related consumer-facing docs or follow-up notes are updated if they should move together
- [ ] Any skipped validation or follow-up work is stated clearly in the final response

If validation was skipped, say so clearly and explain why.

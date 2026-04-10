# Spore Docs

Spore keeps its canonical docs split by concern:

- [../PROTOCOL.md](../PROTOCOL.md): transport, framing, and envelope rules for
  cross-tool MCP traffic
- [internals.md](internals.md): library internals, module boundaries, and
  implementation notes
- [plans/README.md](plans/README.md): active planning entrypoint

Read `PROTOCOL.md` when you are changing wire behavior or shared transport
rules. Read `internals.md` when you are changing the crate structure or shared
library behavior.

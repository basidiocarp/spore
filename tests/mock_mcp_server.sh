#!/bin/sh
# Mock MCP server: reads raw bytes from stdin until we see a closing brace,
# then writes a single JSON-RPC response and exits.
#
# We use dd/head to consume input bytes, avoiding line-buffering issues.

# Read characters one at a time until we see '}'
while true; do
  char=$(dd bs=1 count=1 2>/dev/null) || exit 0
  if [ "$char" = "}" ]; then
    break
  fi
done

# Write response with Content-Length framing
RESPONSE='{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"}]}}'
LENGTH=$(printf '%s' "$RESPONSE" | wc -c | tr -d ' ')
printf "Content-Length: %s\r\n\r\n%s" "$LENGTH" "$RESPONSE"

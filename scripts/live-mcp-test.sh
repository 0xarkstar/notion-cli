#!/usr/bin/env bash
# Live MCP end-to-end test: sends initialize + initialized + tools/call
# (create_data_source) to notion-cli mcp --allow-write against real Notion.
#
# Usage: NOTION_TOKEN=ntn_... ./scripts/live-mcp-test.sh <database_id>
#
# Default database: the one from the earlier smoke test.
set -euo pipefail

DB_ID="${1:-3439867aa14a80b28701cb1cfe1755b5}"
TITLE="mcp-live-$(date +%s)"

cd "$(dirname "$0")/.."

if [[ -z "${NOTION_TOKEN:-}" ]]; then
  echo "NOTION_TOKEN env var required" >&2
  exit 10
fi

REQ=$(cat <<JSON
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"live-test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_data_source","arguments":{"parent_database_id":"${DB_ID}","title":"${TITLE}","properties":{"Name":{"title":{}}}}}}
JSON
)

echo "▶ Database: ${DB_ID}"
echo "▶ Title:    ${TITLE}"
echo "▶ Sending 3 JSON-RPC messages…"
echo "---"

printf '%s\n' "$REQ" | ./target/release/notion-cli mcp --allow-write

# notion-cli

Agent-First Notion CLI and MCP server, written in Rust.

Purpose: replace the broken `@notionhq/notion-mcp-server` whose
`create_a_data_source` tool fails on Notion API 2025-09-03+.

See [DESIGN.md](DESIGN.md) for the architecture record.

## Status

v0.0.1 — pre-release. Will be tagged 0.1.0 once it passes live
Notion API tests against a disposable workspace.

## Build

```
cargo check
cargo test
```

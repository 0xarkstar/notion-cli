# Agent Idempotent Writes

## Why idempotency matters for agents

Agent loops may retry tool calls when a response is lost in transit,
when the agent's context is reset, or when a previous run was
interrupted. Without idempotency keys, retries silently create
duplicate Notion pages.

## Current behaviour (v0.4)

`notion-cli` does not yet forward an `Idempotency-Key` header on
`POST /v1/pages` or `POST /v1/databases`. This means:

- Creating a page twice with identical properties creates two pages.
- There is no built-in deduplication.

**Workaround for agents**: query the target data source for the
candidate page before creating it.

```bash
# Check if a page named "Launch v0.4" already exists
notion-cli ds query <data-source-id> \
  --filter '{"property":"Name","title":{"equals":"Launch v0.4"}}' \
  --page-size 1
# If results is non-empty, skip creation.
```

## Coming in Phase 4

Phase 4 will add `Idempotency-Key` support. The planned interface:

```bash
# CLI: pass a stable key derived from the row's content
notion-cli page create \
  --parent-data-source <ds-id> \
  --properties '{...}' \
  --idempotency-key "import-2026-05-01-row-42"

# MCP: idempotency_key field on create_page
{"tool":"create_page","params":{"parent_data_source_id":"<id>","properties":{...},"idempotency_key":"import-2026-05-01-row-42"}}
```

The key will be forwarded as `Idempotency-Key: <value>` on the Notion
API request. Notion's server deduplicates within a 24-hour window.

## Recommended pattern until Phase 4

1. Derive a stable content hash from the row's canonical fields.
2. Store the `(hash → notion_page_id)` mapping in a local SQLite file
   or the agent's working memory.
3. Before calling `create_page`, check the mapping. If present, call
   `update_page` instead.

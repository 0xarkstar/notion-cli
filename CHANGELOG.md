# Changelog

## 0.2.0 — 2026-04-17

Adds block CRUD — the missing piece for authoring page bodies — plus
distribution infrastructure and actionable error hints.

### Added

- **Block CRUD (12 types)**: `paragraph`, `heading_1`/`_2`/`_3`,
  `bulleted_list_item`, `numbered_list_item`, `to_do`, `toggle`,
  `code`, `quote`, `callout`, `divider`. Same
  `Block { Known | Raw }` pattern as `Property` — unknown block
  types fall through to `Raw` preserving full JSON for read access.
- **5 block endpoints**: `retrieve_block`, `list_block_children`
  (paginated, cursor URL-encoded for safety), `append_block_children`,
  `update_block`, `delete_block`.
- **`page create --children`**: one-shot page creation with body, the
  idiom Notion recommends.
- **CLI verbs**: `notion-cli block {get, list, append, update, delete}`.
- **MCP surface expanded**: read-only now exposes 6 tools
  (4 → +`get_block`, `list_block_children`), `--allow-write`
  exposes 12 tools (7 → +write block ops, audited).
- **Actionable error hints**: `ApiError::Validation` now appends
  one-line remediation for 6 common patterns (wiki data-source,
  missing property, archived parent, type mismatch,
  `object_not_found`, immutable block type).
- **Distribution**: `cargo-dist` configured for 4 targets
  (aarch64/x86_64 × macOS/Linux). GitHub Release on tag push
  produces tarballs + Homebrew formula + shell installer.
- **crates.io metadata**: repository/homepage/documentation URLs,
  refined description, payload trimmed via `exclude`.

### Changed

- `CreatePageRequest` gains `children: Vec<BlockBody>` (default
  empty, omitted on wire when empty).
- MCP `CreatePageParams` gains optional `children: Option<Value>`.
- `NotionClient::delete<T>()` added as a generic method.

### Verification

- 198 tests (up from 130): adds block roundtrip (17), block wiremock
  (13), block handlers (7), block CLI (8), and various small cases.
- `cargo clippy --all-targets -- -D warnings` clean.
- Live-verified against a real Notion workspace — 10-step smoke
  test including 7-block append, list, and delete.

### Migration from 0.1.0

No breaking changes at the wire level. If you construct
`CreatePageRequest` directly in Rust code, add `children: vec![]` to
the literal (or switch to struct update syntax).

## 0.1.0 — 2026-04-15

First release. Replaces `@notionhq/notion-mcp-server` for Notion API
2025-09-03+ workflows, fixing the `create_a_data_source` endpoint
that the upstream incorrectly routed under `/v1/databases/*`.

### What it ships

- **Notion API client** (`src/api/`) — reqwest 0.12 + rustls,
  `governor`-backed 3 req/s rate limiting, 429 retry with
  `Retry-After`, 10 MiB response cap via streaming reads, and
  `Authorization` header scrubbing. `Notion-Version: 2026-03-11`
  pinned.
- **22-variant `PropertyValue` + `Property { Known | Raw }` wrapper**
  with graceful degradation for future property types (works around
  serde issue #912 which blocks `#[serde(other)]` on tagged enums).
- **Newtype IDs** (`DatabaseId`, `DataSourceId`, `PageId`, `BlockId`,
  `UserId`) with two constructors — strict `parse` (format only) and
  URL-accepting `from_url_or_id` — and adversarial validation:
  Unicode homoglyphs, control chars, non-hex, wrong-length inputs are
  rejected.
- **CLI** with six MVP verbs: `db get`, `ds {get,query,create}`,
  `page {get,create,update,archive}`, `search`, `schema <type>`,
  `mcp`. Global flags: `--check-request` (no-network validation),
  `--raw` (skip envelope), `--pretty`, `--token`.
- **MCP stdio server** exposing the same surface with two variants:
  read-only default (4 tools) and `--allow-write` full (7 tools,
  including the-bug `create_data_source`). Append-only JSONL audit
  log on write operations.
- **Untrusted-source envelope** wrapping all Notion-origin output,
  replacing the original DESIGN.md's sanitisation theater.
- **Structured exit codes** (0/2/3/4/10/64/65/74) stable for
  downstream tooling.

### Verification

- 130 tests (id validation, property roundtrip, schema shape, page
  fixture, api wiremock, cli, mcp server, mcp handlers, audit log,
  exit codes), all green.
- Coverage 80.20% lines / 69.94% regions via `cargo llvm-cov`.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo audit` clean (272 deps, 0 advisories).
- Live verification against real Notion workspace:
  - `examples/smoke.rs` — 7-step end-to-end (including the-bug) passes.
  - `scripts/live-mcp-test.sh` — MCP stdio → `create_data_source` →
    real Notion API → 200 OK.

### Deviations from original DESIGN.md

Three architectural pillars of the original design were unsound and
were replaced:

1. **`#[serde(other)] Unknown` on tagged `PropertyValue`** — does not
   compose with `#[serde(tag = "type")]` (serde #912). Replaced with
   `Property { Known(PropertyValue), Raw(serde_json::Value) }` outer
   wrapper using `#[serde(untagged)]`.
2. **Auto-generated schemars output for MCP tool schemas** — emits
   deep `oneOf` + `$ref` recursion for tagged enums and recursive
   filters, degrading agent performance at the MCP boundary. Replaced
   with hand-flattened MCP param structs (IDs as plain strings,
   filter/sorts/properties as `serde_json::Value` with descriptions).
   Schemars output retained for `notion-cli schema <type>`
   introspection only.
3. **Output sanitisation for LLM injection patterns** — theoretically
   unsoundable (regex on natural language is trivially bypassed).
   Replaced with the untrusted envelope: Notion-origin content is
   demarcated as untrusted data, not scrubbed.

Other notable changes:
- `Notion-Version` pinned to `2026-03-11` (DESIGN.md mentioned
  `2025-09-03`, which was stale).
- `DatabaseId::validate` path-traversal check removed — IDs are never
  filesystem paths. Split into strict `parse` + `from_url_or_id`.
- Read-only default on MCP surface — write tools require explicit
  `--allow-write` opt-in.
- YAML config deferred to v0.2 (v0.1 uses `NOTION_TOKEN` env / flag).
- `cargo-dist`, Homebrew tap deferred to v0.2.

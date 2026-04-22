# Changelog

## 0.3.0 — 2026-04-23

Adds the full **admin lifecycle surface** that the v0.2 runtime
boundary left to direct REST calls: database-container creation,
schema mutation, relation wiring, page relocation, user/comment
enumeration. The MCP server gains a third tier (`--allow-admin`) on
top of the v0.2 read-only default and `--allow-write` runtime
writes.

### BREAKING

- **`DataSource::properties` and `Database::properties` type change**
  from `HashMap<String, serde_json::Value>` to
  `HashMap<String, Schema>`. Library consumers that pattern-matched
  on `serde_json::Value` must migrate to matching on
  `Schema::Known(PropertySchema)` / `Schema::Raw(Value)`. The `Raw`
  fallback preserves forward-compatibility: any Notion property-schema
  variant this crate version does not model still round-trips
  losslessly through `Schema::Raw`.

### Added — Admin lifecycle (MCP `--allow-admin`, 4 new MCP tools)

- **`db create`** (`db_create` MCP tool) — `POST /v1/databases` with
  typed `initial_data_source` schema. Parent must be a page (D8 —
  workspace-parent deferred to v0.4 pending OAuth token support).
  Local validation enforces at least one `title`-typed property.
- **`ds update`** (`ds_update` MCP tool) — `PATCH /v1/data_sources/{id}`
  schema mutation with five actions:
  - `add-property` — add a new property schema
  - `remove-property` — destructive (TTY prompts, non-TTY requires
    `--yes`; MCP requires `confirm=true` + `NOTION_CLI_ADMIN_CONFIRMED=1`
    env — two-factor gate per D1)
  - `rename-property` — rename via `{"name": "..."}` directive
  - `add-option` — append a select/multi-select/status option (Notion
    merges by name — existing options preserved)
  - `bulk` — escape hatch for non-atomic multi-delta PATCH (caller
    accepts partial-failure semantics per D2)
- **`ds add-relation`** (`ds_add_relation` MCP tool) — convenience
  wrapper over `ds update` that generates correct
  `dual_property`/`single_property` wire shape with `data_source_id`
  (not `database_id` — forward-compat). Pre-flight GET on target DS
  verifies existence + integration sharing; skipped with `--self`.
- **`page move`** (`page_move` MCP tool) — uses the dedicated
  `POST /v1/pages/{id}/move` endpoint introduced on Notion API
  2026-01-15. `PATCH /v1/pages/{id}` explicitly rejects parent
  mutation. Target accepts `--to-page` or `--to-data-source`.

### Added — CLI-only (intentionally not over MCP in v0.3)

- **`users list/get`** — workspace user enumeration (auto-paginated,
  `--bot-only`/`--human-only` client-side filters). CLI-only per D9
  — workspace PII exfil surface reasons.
- **`comments list/create`** — list/create comments on pages,
  blocks, or existing discussions. CLI-only per D10.

### Added — Runtime-tier additions

- **`page update --icon <emoji|url>` and `--cover <url>`** tristate
  (D11): absent flag leaves unchanged, `--icon none` clears (sends
  JSON `null`), any value sets. `page create` gains the same flags.
  Emoji vs external URL parsing: `http(s)://` prefix → external,
  else emoji literal.

### Added — Agent safety (D1, D3, D5, D6, D13)

- **Three-tier MCP server module split** — `server_ro.rs`,
  `server_write.rs`, `server_admin.rs`, each with its own
  `#[tool_router]` impl sharing `handlers.rs` bodies. Module
  boundary is the invariant: an admin-only tool added to the wrong
  file cannot leak into a lower-privilege tier (D5).
- **MCP tool-list snapshot regression test** — `tests/mcp_server.rs`
  asserts the exact tool set per tier byte-for-byte. Tripwire
  against cross-tier drift (D13).
- **Admin audit log sink** — new `NOTION_CLI_ADMIN_LOG` env
  alongside existing `NOTION_CLI_AUDIT_LOG`. Admin-tool invocations
  route to the admin sink; each entry gains a `privilege` field
  ("write" | "admin") for merge-safe grep/jq (D6).
- **Destructive TTY-aware confirmation** — `std::io::IsTerminal`
  detection: TTY prompts `(y/N)`, non-TTY requires `--yes` (exit 2
  Validation — safety gate, not Usage). MCP equivalent is
  `confirm=true` param PLUS `NOTION_CLI_ADMIN_CONFIRMED=1` env
  (two-factor per D1).
- **Threat-model framing**: `--allow-admin` is **tool-exposure
  policy**, not a security sandbox (D3). An agent with an
  admin-scoped token and code execution can hit the API directly;
  the flag attenuates prompt-injection and accidental action,
  documented explicitly in SKILL.md.

### Added — Shared types

- **`PropertySchema` enum** (22 variants) distinct from
  `PropertyValue`. Wrapped by `Schema { Known | Raw }` untagged
  fallback mirroring v0.2's `Property` pattern. Shares only leaves
  (`SelectOption`, `StatusOption`) to prevent schema-vs-value
  correctness hazards.
- **`Icon` / `Cover` enums** (emoji / external / file variants)
  shared between page/database/data-source objects.
- **`User` / `Comment` types** for the new CLI-only surfaces.
- **`MoveTarget` + `ParentForMove`** enums for `page move`.

### Added — Distribution

- SKILL.md v2 restructured into "Agent tools (MCP)" and "Operator
  CLI" sections. Declares `NOTION_CLI_AUDIT_LOG`,
  `NOTION_CLI_ADMIN_LOG`, `NOTION_CLI_ADMIN_CONFIRMED` in
  `metadata.openclaw.requires.env`. Admin vocabulary is framed as
  least-privilege tool exposure rather than security claim.

### Changed

- `UpdatePageRequest`: `icon` and `cover` now use
  `Option<Option<_>>` tristate (`None` = skip field,
  `Some(None)` = `null` clear, `Some(Some(v))` = set). Library
  consumers constructing the struct directly must add `icon: None,
  cover: None` to existing literals (or use struct-update syntax).
- `CreatePageRequest`: gains `icon: Option<Icon>` and
  `cover: Option<Cover>` (non-tristate — set or omit).
- `CLI mcp` gains `--allow-admin` (mutually exclusive with
  `--allow-write`) and `--admin-log <path>` flags.
- `AuditLog::new_with_admin(write_path, admin_path)` constructor
  added alongside the existing `AuditLog::new(write_path)`.

### Verification

- **280 tests** (up from 198 in v0.2): +82 covering the new surface
  (PropertySchema proptest roundtrip, admin-command wiremock,
  tristate icon/cover, CLI integration, MCP snapshot regression,
  two-sink audit, D1 confirm gate).
- `cargo clippy --all-targets -- -D warnings` clean.
- D12 smoke test: confirmed `POST /v1/pages/{id}/move` per Notion
  docs changelog 2026-01-15.

### Migration from 0.2.0

If you construct `CreatePageRequest` / `UpdatePageRequest`
literals: add `icon: None, cover: None`. If you read
`DataSource.properties` or `Database.properties`: migrate from
`serde_json::Value` matching to `Schema::{Known,Raw}` matching.
Unknown-variant round-trip is preserved via `Schema::Raw`.

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

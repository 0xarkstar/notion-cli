# notion-cli v0.3 handoff (revised post-audit)

> Status: **planned, pre-implementation audit complete (2026-04-22)**.
> v0.2.0 shipped 2026-04-17; issue #1 filed 2026-04-20; audit folded in 2026-04-22.
> Source of truth: https://github.com/0xarkstar/notion-cli/issues/1
> Option **B** confirmed: 7 commands, single v0.3.0 release, ~10h work budget.

## Where we are

- v0.2.0 live on: GitHub (tag + release binaries), crates.io (`notion-cli-mcp`), ClawHub (`0xarkstar/notion-cli-mcp` v1.1.0, Benign/high).
- v0.2.0 covers **agent-facing runtime CRUD**: 22 property types, 12 block types, data-source model (API 2025-09-03+). 198 tests, 80.2% coverage, clippy clean.
- BlueNode workspace bootstrap (1 Wiki + 9 operational DBs + 14 relations) hit the intentional v0.2 boundary: **admin lifecycle ops** (DB container creation, schema mutation, relation wiring) had to go direct against the Notion REST API.
- v0.3 closes that gap. Safety model reframed per audit (see D3).

## Scope (issue #1, revised post-audit)

| # | Command | Endpoint | Size | Priority | MCP exposure |
|---|---------|----------|------|----------|--------------|
| 1 | `db create --parent-page <id> --title --icon --schema <file>` | `POST /v1/databases` (with `initial_data_source`) | M | top | `--allow-admin` |
| 2 | `ds update --add-property / --remove-property / --add-option / --rename` (single-delta per call) | `PATCH /v1/data_sources/{id}` | M-L | high | `--allow-admin` |
| 3 | `ds add-relation --target --backlink \| --one-way \| --self` | convenience wrapper over #2 | S | high | `--allow-admin` |
| 4 | `page move --to-page \| --to-data-source` | `POST /v1/pages/{id}/move` (dedicated endpoint, D12) | S | med | `--allow-admin` |
| 5 | `users list [--bot-only \| --human-only]`, `users get <id>` | `GET /v1/users`, `GET /v1/users/{id}` | S | med | **CLI-only** (MCP deferred to v0.4) |
| 6 | `comments list / create` | `GET/POST /v1/comments` | S | low | **CLI-only** (MCP deferred to v0.4) |
| 7 | `page update --icon <emoji\|url> --cover <url>` (flags, NOT dedicated subcommand) | alias over existing `page update` | S | low | existing `page update` tier (`--allow-write`) |

Non-goals (explicit, unchanged): view management (API gap), `db delete` (use archive), workspace-level db parent, workspace admin.

## Key decisions locked (from pre-implementation audit)

### D1. `--confirm` two-mode semantics
- **CLI**: destructive op prints structured diff, exits 0 without applying. To apply: re-invoke with `--yes`. When stdout is a TTY AND `--yes` absent, prompt interactively `(y/N)`. When non-TTY and `--yes` absent, exit 2.
- **MCP**: destructive admin tools take explicit `confirm: bool` param (must be `true`). ALSO gate on env var `NOTION_CLI_ADMIN_CONFIRMED=1` — two factors. Agent config alone cannot mutate schema without both present.
- Rationale: resolves the CLI-vs-MCP contradiction that crashed naive "interactive confirm" plan. Agents use explicit-flag path; humans get tty prompt safety net.

### D2. `ds update` atomicity — single-delta default
- Default: CLI accepts **one property change per invocation** (rejects multi-delta). Sequential execution through the 3 req/s limiter; stop on first failure; print progress.
- Escape hatch: `--bulk` flag sends multi-delta PATCH — caller accepts non-atomic mid-state on partial failure.
- Library API `data_source::update_data_source(id, req)` remains multi-delta capable for advanced consumers.
- Rationale: Notion PATCH is not transactional; partial failure leaves schema in mixed state with no rollback primitive. Sequential-by-default trades latency for auditability.

### D3. `--allow-admin` threat model (REFRAME — NOT a sandbox)
- `--allow-admin` is **tool-exposure policy**, NOT a security boundary. An agent with admin integration token + code execution can call `POST /v1/databases` via curl regardless of MCP gating.
- What it DOES do:
  1. **Prompt-injection attenuation**: admin tools not in agent's tool menu → excluded from agent's planning surface.
  2. **Accidental-action prevention**: default Hermes profiles have no admin tools → operator cannot fat-finger schema changes through a read/write-only agent.
- SKILL.md must state this explicitly ("least-privilege default for agent tool menus"). Do NOT imply sandbox-against-hostile-agent.

### D4. `PropertySchema` distinct from `PropertyValue`
- New file `src/types/property_schema.rs`. Mirror the proven pattern: `PropertySchema::Known(PropertySchemaKind)` with `#[serde(tag = "type")]`, wrapped by `Schema { Known | Raw }` untagged outer for graceful degradation.
- Share **only leaves** (`SelectOption`, `StatusOption` from `src/types/common.rs`).
- Each variant carries a configuration struct (e.g. `SelectSchema { options: Vec<SelectOption> }`, `RelationSchema { data_source_id: DataSourceId, relation_type: RelationType }`).
- **Breaking change**: `DataSource.properties` and `Database.properties` migrate from `HashMap<String, serde_json::Value>` to `HashMap<String, Schema>`. Note in CHANGELOG under BREAKING. Pre-1.0 semver permits.
- MCP tool params stay `serde_json::Value` with example-rich descriptions (v0.2 lesson on schemars deep-recursion — agents parse prose better than recursive `$ref`).
- Proptest roundtrip mandatory: per-variant serialize + deserialize lossless.

### D5. MCP three-tier module split
- Three files, three `#[tool_router]` impls, one shared `src/mcp/handlers.rs` module for bodies.
  - `src/mcp/server_ro.rs` — `NotionReadOnly` (6 tools, unchanged from v0.2)
  - `src/mcp/server_write.rs` — `NotionWrite` (12 tools: v0.2's 12 runtime tools, unchanged)
  - `src/mcp/server_admin.rs` — `NotionAdmin` (12 + 5 admin = 17 tools: `db create`, `ds update`, `ds add-relation`, `page move`, `page update` with admin flag. Users/comments CLI-only.)
- Module boundary is the invariant. An admin-only tool accidentally added to `NotionWrite` should ideally fail compilation; at minimum be caught by D13 snapshot.

### D6. Audit log — two env vars (separate files)
- `NOTION_CLI_AUDIT_LOG` — runtime writes (v0.2 behaviour, unchanged).
- `NOTION_CLI_ADMIN_LOG` — admin ops (new, higher-privilege).
- Rationale: operator can grep-split agent activity vs structural mutation without jq filters; different retention/rotation policies possible.
- Both declared in `clawhub/SKILL.md` `metadata.openclaw.requires.env` (per ClawHub pre-publish checklist below).

### D7. `ds add-relation` — minimal pre-flight, explicit flag choice
- Pre-flight: single GET on target DS. Verify (exists, not a wiki container, shared with integration). Map errors to targeted hints. No graph traversal, no backlink name uniqueness inference.
- Flag choice must be explicit: exactly one of `--backlink <name>` / `--one-way` / `--self`. No silent default.
- `--self` required when `<source_ds> == <target_ds>` (surface intent; self-relations are unusual — avoid typo footgun).
- Uses `data_source_id` exclusively (NOT `database_id` — forward-compat trap on API 2025-09-03+).

### D8. `db create` parent = page only (no workspace)
- `CreateDatabaseParent` enum = `Page { page_id: PageId }` only for v0.3.
- Workspace-parent requires OAuth user tokens that integration tokens lack. Shipping it produces opaque 400s.
- Add when/if OAuth token support lands in v0.4+.
- Validate locally via `--check-request`: parent is `PageId`, at least one property is `Title`-typed, property names unique.

### D9. `users list` auto-paginate, CLI-only
- Default: walk cursors until exhausted, `page_size=100` (API max).
- Flags: `--limit <n>` (client-side cap), `--bot-only` / `--human-only` (client-side filter on `type`), `--cursor <c>` (manual escape hatch).
- **CLI-only in v0.3** (NOT exposed over MCP). Reduces PII-exfil surface and ClawHub scanner load.
- Reconsider MCP exposure in v0.4 only if a real agent use-case emerges.

### D10. `comments` CLI verb shape, CLI-only
- `comments list --on-page <id> | --on-block <id>` — exactly one parent (mutually exclusive).
- `comments create --on-page <id> | --on-block <id> | --in-discussion <id> --text "<body>"` — exactly one parent.
- **CLI-only in v0.3** (NOT exposed over MCP). If MCP demand emerges in v0.4, add a separate slower rate-limit bucket (e.g. 1/5s) for comment creation at that time.
- Do NOT invent a fake "reply-to comment id" — Notion's model is discussion-based, not reply-hierarchy.

### D11. `page icon/cover` — flags on `page update`, no dedicated subcommand
- Extend `UpdatePageRequest` with `icon: Option<Icon>`, `cover: Option<Cover>` (and `parent: Option<PageParent>` per D12).
- `--icon <emoji|url>` parse rule: `http(s)://` prefix → `external { url }`; else emoji → `{ type: "emoji", emoji: "<v>" }`.
- `--cover <url>` only (Notion covers are URL-only).
- Clearing: `--icon none` / `--cover none` → sends `null` in body.
- One shared helper in `api/page.rs` routes the flags AND any future dedicated shortcut — divergence risk ≈ 0.

### D12. `page move` via dedicated endpoint (smoke test PASSED 2026-04-22)
- **Smoke test finding**: Notion introduced `POST /v1/pages/{page_id}/move` on 2026-01-15. `PATCH /v1/pages/{id}` explicitly rejects parent mutation ("A page's parent cannot be changed"). The dedicated move endpoint is the correct surface.
- **Do NOT extend `UpdatePageRequest`** with `parent` — previous handoff's approach is obsolete. Add a new function `move_page(id, target)` to `src/api/page.rs` that POSTs to the move endpoint.
- Body shape: `{"parent": {"type": "page_id" | "data_source_id", "...": "<id>"}}`. Target types supported: `page_id`, `data_source_id`. Use `data_source_id` not `database_id` (forward-compat with 2025-09-03+ data-source migration).
- CLI: `page move <page_id> --to-page <id> | --to-data-source <id>` (mutually exclusive).
- Restrictions to document in error hints: must be a regular page (not database), bot needs edit access on new parent, cross-workspace moves rejected.
- **Size: S** (original estimate, after smoke-test clarified scope — no UpdatePageRequest extension needed).

### D13. MCP tool-list snapshot regression test
- `tests/mcp_server_snapshot.rs` — start server in each tier, assert `tools/list` returns exactly the expected tool names/order:
  - no-flag → 6 tools (v0.2 RO baseline)
  - `--allow-write` → 12 tools (v0.2 runtime baseline, byte-for-byte)
  - `--allow-admin` → 17 tools (v0.2's 12 + 5 new admin tools)
- Trips on any accidental cross-tier addition. Invariant test, not a unit test. Defends the D5 module boundary from reviewer error.

## ClawHub pre-publish checklist

- **Dry-run SKILL.md v2.0** against OpenClaw rules BEFORE publishing (copy to scratch slug if needed).
- SKILL.md structural split:
  - "Agent tools (MCP)" section lists ONLY read + runtime-write tools
  - "Operator CLI" section describes admin ops with explicit "not available over MCP by default; opt-in per deployment via `--allow-admin`" preamble
- `metadata.openclaw.requires.env`: `NOTION_TOKEN`, `NOTION_CLI_AUDIT_LOG`, `NOTION_CLI_ADMIN_LOG`.
- `metadata.openclaw.capabilities`: declare admin tier explicitly.
- Budget **one iteration** (Suspicious → Benign) into release timeline. Not exceptional — expected.

## Testing strategy (revised +70-90 tests, up from +40-60)

Scope revised up after audit. 7 commands × richer state (diff preview, confirm logic, 3-tier gating) demand more coverage.

- **Unit**: property schema roundtrip (proptest, per variant), diff formatting snapshot, icon/cover parse rules, `--check-request` structural validation
- **wiremock**: all 7 new endpoints (200/400/404/429), parent-page-not-found → `object_not_found` hint, relation-target-not-shared hint, wiki-parent-for-relation hint, `synced_property_name` collision hint
- **CLI assert_cmd**: `--check-request` coverage of every new subcommand, TTY vs non-TTY confirm paths, `--yes` gating (exit 2 when missing in non-TTY), `--bulk` gating (exit 3 on partial failure)
- **Live smoke** (extend `examples/smoke.rs`): create a test DB with 2 properties, add a relation to an existing DS (dual-property), move a page (dual-branch: to-page AND to-data-source), remove a property, verify diff output
- **MCP snapshot (D13)**: byte-compare tool-list JSON per tier
- **MCP gating**: admin tools NOT in `tools/list` without `--allow-admin`; admin tool call WITHOUT `confirm: true` + `NOTION_CLI_ADMIN_CONFIRMED=1` returns error (D1 invariant)
- **Partial-failure**: `ds update --bulk` with forced mid-response failure → exit 3 + structured report listing applied-vs-failed deltas

Target: +70-90 tests on v0.2's 198 → ~270-290 total. Maintain 80%+ line coverage.

## Distribution checklist

1. Bump Cargo.toml: 0.2.0 → 0.3.0
2. Update CHANGELOG with:
   - **BREAKING**: `DataSource.properties` and `Database.properties` type migration (D4)
   - New commands (1-7 above)
   - New env vars (`NOTION_CLI_ADMIN_LOG`, `NOTION_CLI_ADMIN_CONFIRMED`)
   - New MCP flag tier (`--allow-admin`)
3. Update README features list (12 → 17 MCP tools, CLI gains 7 admin subcommands)
4. Tag v0.3.0 → cargo-dist rebuilds 4-platform binaries (~6 min)
5. `cargo publish` to crates.io
6. `clawhub publish ./clawhub --version 2.0.0 ...` (major bump; admin section added). Expect one Suspicious→Benign iteration per ClawHub checklist above.

## Open questions for implementer

1. ~~D12 smoke test~~ — DONE 2026-04-22. Finding: dedicated `POST /v1/pages/{page_id}/move` endpoint (2026-01-15 release). See D12 for corrected design.
2. Separate binary `notion-cli-admin`? Would simplify ClawHub scanner story (no admin vocabulary in the public CLI SKILL). **Defer post-v0.3** — revisit only if ClawHub iteration proves painful.
3. `--reconcile` mode for `ds update` (whole-schema JSON reconciliation): **defer to v0.4+**.
4. `ds update --bulk` partial-failure exit code: `3` with structured report (this plan's default), or `2` + separate `--continue-on-error` flag? Pick at implementation time, document in CHANGELOG.

## Pre-implementation audit (DONE 2026-04-22)

Architect + critic ran in parallel. Key findings folded into D1-D13 above. Original audit outputs archived in session transcript (agents `aa6bfb68989e007f4` and `aaf079d98d42a785c`).

**Must-fix items all addressed in this revision:**
- ~~Handoff line 103 file pointer bug~~ → fixed in D12
- ~~`--confirm` CLI-vs-MCP contradiction~~ → resolved in D1
- ~~`ds update` non-atomicity unmitigated~~ → resolved in D2
- ~~`--allow-admin` framed as security sandbox~~ → reframed in D3 as tool-exposure policy
- ~~`users list` / `comments create` MCP exposure~~ → deferred to v0.4 in D9/D10
- ~~MCP regression risk~~ → snapshot test in D13
- ~~Test budget 40-60 too tight~~ → raised to 70-90
- ~~ClawHub pre-publish optimism~~ → explicit checklist added

## File pointers (corrected)

- `src/types/property_schema.rs` — **NEW** (D4)
- `src/types/data_source.rs` — update `properties: HashMap<String, serde_json::Value>` → `HashMap<String, Schema>` (D4, BREAKING)
- `src/types/database.rs` — same (D4)
- `src/api/data_source.rs` — extend with `update_data_source(id, req)` + single-delta shape + `--bulk` (D2)
- `src/api/database.rs` — add `create_database(req)` with typed `PropertySchema` (D4, D8)
- `src/api/page.rs:46-53` — **EXTEND `UpdatePageRequest`** with `icon: Option<Icon>`, `cover: Option<Cover>` (D11 only). Do NOT add `parent` — the move endpoint is separate.
- `src/api/page.rs` — **NEW function** `move_page(id, MoveTarget)` posting to `/v1/pages/{id}/move` (D12). New types: `MoveTarget { ToPage(PageId), ToDataSource(DataSourceId) }` + `ParentForMove` serde-tagged enum for the body.
- `src/api/user.rs` — **NEW** (D9, CLI-only plumbing)
- `src/api/comment.rs` — **NEW** (D10, CLI-only plumbing)
- `src/api/error.rs:54-106` — extend validation-hint registry (relation-target-not-shared, wiki-parent-for-relation, synced_property_name collision, parent-page-not-found for `db create`)
- `src/cli/db.rs`, `src/cli/ds.rs`, `src/cli/page.rs`, `src/cli/user.rs`, `src/cli/comment.rs` — new subcommands
- `src/mcp/server_ro.rs` / `server_write.rs` / `server_admin.rs` — three-file split (D5)
- `src/mcp/handlers.rs` — shared handler bodies (D5)
- `src/mcp/audit.rs` — add `NOTION_CLI_ADMIN_LOG` sink alongside existing write log (D6)
- `clawhub/SKILL.md` — restructure into agent-tools / operator-CLI sections per D3 + D11 + checklist
- `tests/mcp_server_snapshot.rs` — **NEW** (D13)
- `tests/property_schema_roundtrip.rs` — **NEW** (D4 proptest)

## Implementation order (strict)

1. ~~D12 smoke test~~ — DONE. Dedicated `POST /v1/pages/{page_id}/move` exists; `UpdatePageRequest` unchanged for move (see D12).
2. **Type foundation (D4)** — `PropertySchema` enum + `Schema { Known | Raw }` wrapper + proptest roundtrip. Migrate `DataSource.properties` / `Database.properties`. Verify v0.2 read shape still deserialises via `Raw` fallback.
3. **MCP three-file split (D5)** — refactor BEFORE adding any new tool. D13 snapshot test as safety net confirming v0.2 surface unchanged.
4. **`db create` (#1)** — exercises PropertySchema write path first.
5. **`ds update` single-delta (D2, #2)** — four subcommands: add-property, remove-property, add-option, rename.
6. **`ds add-relation` (#3)** — convenience over #2.
7. **Extend `UpdatePageRequest` (D11, D12)** — `icon`, `cover`, `parent` fields.
8. **`page update --icon/--cover` (#7)** — flags on existing update.
9. **`page move` (#4)** — admin-tier wrapper, both `--to-page` and `--to-data-source` branches.
10. **`users list/get` (#5)** — CLI-only, no MCP plumbing.
11. **`comments list/create` (#6)** — CLI-only, no MCP plumbing.
12. **Destructive confirm (D1)** — retrofit to #2 (remove-property, rename), #4 (to-trash path if any). MCP param + env-var gating.
13. **ClawHub SKILL.md restructure + dry-run (D3, ClawHub checklist)**.
14. **Release** — v0.3.0 + ClawHub v2.0 + CHANGELOG BREAKING note.

## When done

- Close issue #1
- Update memory `cli-development-patterns.md` with admin-ops lessons (esp. PropertySchema, 3-tier module split, two-mode confirm)
- Add Obsidian Patterns/ note: `cli-agent-admin-three-tier.md` (per `~/Obsidian/Dev/Inbox/notion-cli-admin-vs-agent-boundary-2026-04-22.md` extraction pointer)
- Add Obsidian inbox note only if new reusable patterns emerged beyond what audit captured

---

Drafted 2026-04-22 end of v0.2 post-release session.
Revised 2026-04-22 post-audit (architect + critic parallel, 13 decisions locked).

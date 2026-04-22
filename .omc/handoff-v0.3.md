# notion-cli v0.3 handoff

> Status: **planned, not started**. v0.2.0 shipped 2026-04-17; issue #1 filed 2026-04-20.
> Source of truth: https://github.com/0xarkstar/notion-cli/issues/1

## Where we are

- v0.2.0 live on: GitHub (tag + release binaries), crates.io (`notion-cli-mcp`), ClawHub (`0xarkstar/notion-cli-mcp` v1.1.0, Benign/high).
- v0.2.0 covers **agent-facing runtime CRUD**: 22 property types, 12 block types, data-source model (API 2025-09-03+). 198 tests, 80.2% coverage, clippy clean.
- BlueNode workspace bootstrap (1 Wiki + 9 operational DBs + 14 relations) hit the intentional v0.2 boundary: **admin lifecycle ops** (DB container creation, schema mutation, relation wiring) had to go direct against the Notion REST API.
- v0.3 closes that gap. Safety model unchanged.

## Scope (issue #1)

| # | Command | Endpoint | Size | Priority |
|---|---------|----------|------|----------|
| 1 | `db create --parent-page <id> --title --icon --schema <file>` | `POST /v1/databases` (with `initial_data_source`) | M | top |
| 2 | `ds update` (add/remove property, add select option, rename) | `PATCH /v1/data_sources/{id}` | M-L | high |
| 3 | `ds add-relation --target --backlink \| --one-way` | convenience wrapper over #2 | S | high |
| 4 | `page move --to-page \| --to-data-source` | `PATCH /v1/pages/{id}` | S | med |
| 5 | `users list [--bot-only\|--human-only]`, `users get <id>` | `GET /v1/users`, `GET /v1/users/{id}` | S | med |
| 6 | (optional) `comments list/create` | `GET/POST /v1/comments` | S | low |
| 7 | (optional) `page icon/cover` — dedicated shortcuts | alias over `page update` | S | low |

Non-goals (explicit): view management (API gap), `db delete` (use archive), workspace admin.

## Recommended implementation plan

**Option A — core 5 (commands 1–5), single v0.3.0 release** (~6–8h)

Covers the full BlueNode bootstrap scenario. Options 6–7 defer to v0.3.1+.

**Option B — A + optional 2** (~10h)

Seals the admin surface completely.

**Option C — phased incremental** (v0.3.0 = 1+2+3, v0.3.1 = 4+5, v0.3.2 = 6+7)

Ship-early path if the Phase 2–4 bootstrap use case is most urgent.

Default recommendation: **Option A**.

## Design considerations — carry-over from v0.1/v0.2

1. **Admin vs runtime separation**. MCP tool exposure was designed for agent CRUD; admin ops (esp. schema mutation) should **not** be exposed over MCP by default — agents pairing with Hermes shouldn't be able to drop a property. Gate admin tools behind an additional flag (e.g. `--allow-admin`, stricter than `--allow-write`). Keep MCP read-only default untouched.

2. **Destructive confirmation at CLI**. Every property removal, data-source/database rename, page move into an ancestor, or move-to-trash must require `--confirm` (print diff, require explicit yes). Dry-run still available via `--check-request`.

3. **Property schema modelling — same Property pattern**. Property *schemas* (what `ds update --add-property` sends) are a different shape than property *values* (what `page create` sends):
   - Schema: `{"Priority": {"select": {"options": [{"name":"긴급"}, ...]}}}`
   - Value: `{"Priority": {"select": {"name": "긴급"}}}`
   - Both live in the same `properties` field on the wire depending on endpoint. Model as distinct Rust types to keep compile-time separation.

4. **Schema diff output**. `ds update` should print a structured diff before applying:
   ```
   ~ Tags (multi_select):
     + 아카이브
     + 검토필요
   + Priority (select, new): 긴급, 높음, 보통
   - old_field (removed)
   ```

5. **Relation wiring — the hand-crafted pain point**. `ds add-relation` must generate correct `dual_property` or `synced_property_name` with target `data_source_id`. Notion rejects relation creation if the target DS is in a different workspace or not shared with the integration — surface that as a specific error hint.

## Testing strategy (mirror v0.2 pattern)

- **Unit**: property schema roundtrip (proptest), diff formatting snapshot
- **wiremock**: all 5–7 new endpoints (200/400/404/429), parent-page-not-found → object_not_found hint
- **CLI assert_cmd**: --check-request coverage of every new subcommand
- **Live smoke** (extend `examples/smoke.rs`): create a test DB with 2 properties, add a relation to an existing DS, move a page, remove a property, verify diff
- **MCP**: admin tools gated behind --allow-admin; verify they are NOT in tools/list without it

Target: add ~40–60 tests on top of v0.2's 198, maintain 80%+ line coverage.

## Distribution checklist (unchanged from v0.2)

1. Bump Cargo.toml: 0.2.0 → 0.3.0
2. Update CHANGELOG with new commands
3. Update README features list (12 → 17 or 19 tools)
4. Tag v0.3.0 → cargo-dist rebuilds 4-platform binaries (~6 min)
5. `cargo publish` to crates.io
6. `clawhub publish ./clawhub --version 2.0.0 ...` (major-bump since SKILL.md gains admin section)

## Open questions for implementer

1. Should admin ops also get an audit log (JSONL), same path as writes? Suggest **yes** — these are higher-privilege than regular writes.
2. Should `ds update` accept a full schema JSON file for idempotent "reconciliation" mode? (Advanced; defer to v0.4 if feature creep.)
3. ClawHub SKILL.md update — new admin section should explicitly note `--allow-admin` is agent-hostile default-off. Scanner may flag admin ops as higher-privilege; expect to iterate to Benign again.

## Pre-implementation audit (recommended, per v0.2 pattern)

Before writing code, spawn 1-2 parallel audit agents (architect + critic) to stress-test:
- Property schema Rust modelling (distinct from PropertyValue)
- `ds update` atomicity (Notion API is not transactional — partial failures possible on multi-change calls)
- `db create` parent-shape edge cases (page vs database parent; workspace_id only for admin)
- `ds add-relation` one-way vs dual vs self-referential

Follow the audit → synthesize → implement → review → live → release flow proven on v0.2.

## File pointers

- `src/api/data_source.rs` — extend with `update_data_source(id, req)` + schema types
- `src/api/database.rs` — add `create_database(req)`
- `src/api/page.rs` — `update_page` already supports `parent` patch; thin CLI layer
- `src/cli/ds.rs`, `src/cli/db.rs`, `src/cli/page.rs` — new subcommands
- `src/api/user.rs` — new module
- `src/mcp/*` — conditional admin tools (skip in read-only and --allow-write modes unless --allow-admin)

## When done

- Close issue #1
- Update memory `cli-development-patterns.md` with admin-ops lessons (esp. schema-value type split)
- Add Obsidian inbox note only if new reusable patterns emerged

---

Drafted 2026-04-22 at end of v0.2 post-release session.

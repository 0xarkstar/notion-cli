# notion-cli v0.4 handoff (revised post-audit)

> Status: **planned, pre-implementation audit complete (2026-04-23)**.
> v0.3.0 shipped 2026-04-23 (crates.io + GitHub binaries + ClawHub v2.0.1 CLEAN).
> Source of truth: this file. v0.3 precedent: `.omc/handoff-v0.3.md`.
> Target: minor bump, **non-BREAKING for JSON consumers**, additive only, **12-14 work days** (widened from 8-10 per audit).

## Theme

**"Justin Poehnelt agent-first CLI 11 원칙 완전 정렬 + L2 관측/재현 완성도 + 누락 API 3종 보완."**

L3 (multi-tenant gateway / OAuth user tokens / webhook daemon) 미터치.

## Where we are

- v0.3.0 covers admin lifecycle ops: db create / ds update / ds add-relation / page move + CLI-only users/comments. **3-tier MCP: RO 6 / write 12 / admin 16**.
- 280 tests, clippy clean, cargo audit clean (RUSTSEC-2026-0104 patched via rustls-webpki 0.103.13).
- v0.4 closes the **agent-first CLI gaps** from DESIGN.md reference to Justin Poehnelt's *"Rewrite your CLI for AI Agents"* blog post. Matrix audit found 1 A-grade gap (Principle #1 Universal `--json`) + observability/reproducibility shortfalls.

## Audit-derived BLOCKER resolutions

**B1: `PATCH /v1/databases/{id}` parent mutation**
- **Verified 2026-04-23 via Notion docs**: PATCH accepts `parent` with `page_id` OR `workspace: true`. Full accepted fields: `parent`, `title`, `description`, `is_inline`, `icon`, `cover`, `in_trash`, `is_locked`. No dedicated `/move` endpoint exists for databases (unlike pages).
- **D8 precedent**: v0.3 locked `db create --parent-workspace` as integration-token blocked. The same integration-token restriction likely applies to `db update --to-workspace`. Per E10 below: document possibility + map 403 to targeted hint, do not block at parse time.

**B2: MCP tool description with JSON examples**
- **Verified via rmcp-macros 1.4.0 source**: `#[tool(description = ...)]` parses via `darling::FromMeta` → `LitStr` → **rejects `include_str!` macro expression**.
- **Fallback pattern works**: `#[doc = include_str!("...")]` on the tool function → `extract_doc_line` fallback (`tool.rs:321`) preserves the macro. Test `test_doc_include_description` (`tool.rs:442-458`) confirms.
- **Decision**: Use `#[doc = include_str!("../../docs/cookbook/snippets/<tool>.md")]` on each admin/new MCP tool function. No dependency on `#[tool(description=...)]`.

## Scope (4 surfaces — strictly separated)

### 🟢 CLI surface (humans + scripts)

| # | Feature | Notes |
|---|---------|-------|
| C1 | Universal `--json <body>` on all mutation commands | Justin #1 — zero-translation path. Reads stdin (`-`), `@file`, or literal. |
| C2 | `--stream` / `--format=jsonl` on 5 paginated commands | Justin #4 — NDJSON with explicit end-frame. |
| C3 | `--dry-run` alias for `--check-request` | Justin #6 — naming alignment. |
| C4 | `--check-request --cost` | API call count + rate-limit window estimate. |
| C5 | `db update <id>` — `title` / `description` / `icon` / `cover` / `--to-page` / `--to-workspace` / `--in-trash` / `--is-locked` / `--is-inline` | Missing API. `PATCH /v1/databases/{id}`. `--to-workspace` warned at runtime per E10. |
| C6 | `users me` + `auth whoami` alias | Missing API. `GET /v1/users/me`. `auth` namespace reserved for v0.6 OAuth. |
| C7 | `page get --properties <ids>` | Justin #3 — field-mask. `filter_properties` query param. |
| C8 | `page get-property <page> <prop-id>` | Missing API. `GET /v1/pages/{page}/properties/{prop_id}`. **Shape varies**: scalar types return object; list-valued (`relation`, `rollup`, `people`, `title`, `rich_text`) return paginated list. |

### 🟣 MCP surface (agent runtime)

| # | Feature | Notes |
|---|---------|-------|
| M1 | Request correlation ID (UUID v7) in audit + envelope + error | E1; cross-system trace |
| M2 | GET response LRU cache — `NOTION_CLI_CACHE_TTL=30s` (default OFF) | E2; writes invalidate |
| M3 | Idempotency-Key auto-gen on write/admin destructive + MCP override | E3 |
| M4 | `users_me` tool at RO tier (inherited by write + admin) → **RO 7 / write 13 / admin 18** | Bot self-id; D9 exception (see §D9-ex) |
| M5 | `db_update` tool at admin tier → contributes to **admin 18** | Mirrors C5 |
| M6 | `json` param on write/admin tools — bespoke params kept in parallel; **reject on mixed** | E8 |

**MCP tool count after v0.4: RO 7 / write 13 / admin 18** (v0.3 baseline: 6 / 12 / 16; v0.4 adds `users_me` to all 3 tiers + `db_update` to admin).

**D9 exception for `users_me`**: v0.3 D9 kept `users list/get` CLI-only because those enumerate workspace PII. `users_me` returns only the bot's own identity (from the integration token) — no PII enumeration surface. Exception granted. **D9 still binds `users_list` and `users_get`** — they stay CLI-only.

### 🟡 Skills surface (agent learning)

| # | Feature | Notes |
|---|---------|-------|
| S1 | MCP tool descriptions include JSON body examples via `#[doc = include_str!(...)]` | B2-resolved doc-comment fallback |
| S2 | `docs/cookbook/` — 4 canonical workflows | bootstrap-workspace, bulk-import-csv, reconcile-schema, agent-idempotent-writes |
| S3 | `docs/runtime-samples/` — 3 sample configs | Hermes / Claude Desktop / Cursor; explicit `# SAMPLE ONLY` header |
| S4 | Error-hint registry +6 patterns | relation-unshared, wiki-parent, synced-name-collision, move-restrictions, integration-workspace-403, property-filter-id-404 |
| S5 | `clawhub/SKILL.md` v2.0.1 → v2.1.0 | Canonical vs sample boundary |
| S6 | `tests/cookbook_examples.rs` wiremock gate | Validates every JSON snippet in `docs/cookbook/snippets/` round-trips against current client types (prevents rot — E7) |

### ⚙️ Shared kernel (CLI + MCP common)

| # | Feature | Notes |
|---|---------|-------|
| K1 | `tracing` + `tracing-subscriber` with request_id propagation | Structured log baseline |
| K2 | OTel exporter behind feature flag `otel` | E5; `--otlp-endpoint` flag |
| K3 | `TokenProvider` trait in `src/token_provider/` (renamed from `src/auth/` per M1) — env / file / keychain (macOS only, `cfg(target_os="macos")`) / exec chain + **shadowing warning** | E6 |
| K4 | `Cache` trait + default LRU; **writes invalidate** page/data_source/block entries | M2 backing |

---

## Locked decisions E1–E12 (revised)

| ID | Decision |
|----|----------|
| E1 | `request_id` = UUID v7 (time-sortable). Added as **last** field of `AuditEntry` struct (layout churn minimized). Non-BREAKING for JSON-key consumers; **positional / byte-layout parsers must update** — documented in CHANGELOG. |
| E2 | GET cache default **OFF**. Activates only when `NOTION_CLI_CACHE_TTL` set. **Writes invalidate** cache entries for modified entity (page_id / data_source_id / block_id). `--check-request --cost` estimates cold cache (conservative). |
| E3 | Idempotency key = client UUID v4, `Idempotency-Key` HTTP header. MCP `idempotency_key` param overrides. Sent even to non-honoring endpoints (HTTP semantics: unknown headers safely ignored). |
| E4 | `--stream` / `--format=jsonl` frames: `{"type":"item","content":{...}}` per row, `{"type":"end","cursor":null}` on clean finish, `{"type":"error","at_cursor":"...","code":"...","message":"..."}` on mid-stream failure followed by halt (no further items). Exit code 1 on error frame. |
| E5 | OTel gated by cargo feature `otel`. Default build excludes it (deps: `opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry` all under feature). |
| E6 | `TokenProvider` = ordered chain (env → file → keychain → exec). First Ok wins. **Multi-source shadowing warning**: when env is Ok AND any later provider is also Ok, emit stderr warning `notion-cli: warning: NOTION_TOKEN env var shadows <provider> entry; using env`. `NOTION_CLI_TOKEN_CHAIN` env overrides order. |
| E7 | Skills **canonical** = `clawhub/SKILL.md` + MCP tool doc-strings + `docs/cookbook/`. Cookbook JSON snippets gated by `tests/cookbook_examples.rs` (S6) — rot prevented. Other = sample. |
| E8 | `--json <body>` + bespoke flag (`--title`, `--icon`, etc.) → **reject with exit 2** + error hint `"--title ignored when --json present; remove one"`. **NOT silent drop** (Karpathy antipattern). Exception: read-only output shaping (`--format`, `--pretty`) can coexist. |
| E9 | `--dry-run` is internal alias for `--check-request`. Both work; docs lead with `--check-request`. |
| E10 | `db update --to-workspace` — documented as "requires OAuth user token; integration tokens typically return 403." Error-hint registry entry wires 403 → `"Workspace-level moves require a user OAuth token; integration tokens cannot perform this move."` |
| E11 | MCP tool JSON examples injected via `#[doc = include_str!("../../docs/cookbook/snippets/<tool>.md")]` on tool functions (rmcp-macros doc-comment fallback). NOT `#[tool(description=...)]` (darling rejects macro expressions). |
| E12 | `--json <body>` body goes through `--check-request` local validation (endpoint-shape assertion) before send. Prevents `--json` from routing around v0.3's local-validation wins. |

---

## Explicit non-goals

| Exclusion | Where | Reason |
|-----------|-------|--------|
| OAuth user-token flow | v0.6 | L3 multi-tenant |
| Webhook consumer | v0.6 | Independent binary |
| Multi-tenancy | v0.7+ | L3 |
| File uploads | v0.5 | Presigned multi-part |
| Prometheus `/metrics` sidecar | v0.6+ | HTTP listener = L3 |
| Query DSL (`--where`) | v0.5 candidate | `--json` handles it for v0.4 |
| MCP `resources` primitive | v0.5 candidate | Separate design |
| Global `--quiet` flag | **never** | Intentional divergence (`--yes` per-op safety) |
| Extra agent-runtime samples | PR-driven only | Maintenance burden |
| Linux/Windows keychain providers | v0.5+ | macOS-only via platform-gated dep |

---

## 4-phase implementation (12-14 days)

### Phase 1 — Missing API + Skills relocation (3 days, was 2)

**First commit (snapshot-safe sequencing):** update `tests/mcp_server.rs` — RO 6→7, write 12→13 (`users_me` inherited), admin 16→18 (`users_me` + `db_update`). Snapshot update lands together with tool registration in one commit to avoid mid-branch CI breakage.

- `db update` — `src/api/database.rs::update_database`, `src/cli/db.rs::Update`, `src/mcp/server_admin.rs::db_update`. Parent mutation + all 8 fields.
- `users me` + `auth whoami` alias — `src/cli/user.rs`, `src/mcp/server_ro.rs::users_me`.
- `page get --properties <ids>` — extend `RetrievePageParams` with `filter_properties: Vec<String>`.
- `page get-property <page> <prop-id>` — new endpoint + **shape dispatch** (scalar vs list-valued).
- `docs/cookbook/` 4 files + `docs/cookbook/snippets/` per tool (`db_create.md`, `db_update.md`, `ds_update.md`, `ds_add_relation.md`, `page_move.md`, `users_me.md`).
- `docs/runtime-samples/` 3 files with `# SAMPLE ONLY — NOT ENFORCED` header.
- `clawhub/SKILL.md` v2.1.0 restructure.
- **+18 tests** (3 per new API × 3 paths — happy + validation + error; + 3 snapshot test updates; + 3 shape-dispatch for page get-property).

### Phase 2 — Universal `--json` + streaming (3 days, was 2)

- 7 mutation CLI commands gain `--json <body>` with stdin (`-`) / `@file` / literal parsing.
- MCP write/admin tools gain optional `json` param.
- **E8 rejection**: `--json` + any bespoke-body flag → exit 2 with hint. Each command has `conflicts_with_json` test.
- **E12 validation**: `--json` body runs through `--check-request` local validator before HTTP.
- `--stream` + `--format=json|jsonl` on 5 paginated commands (`ds query`, `page children-list`, `search`, `users list`, `comments list`).
- `--dry-run` alias wiring.
- **+25 tests** (7 `--json` × 3 paths + 5 `--stream` + 7 reject-mixed + 5 end-frame + 1 error-frame).

### Phase 3 — Observability (3-4 days, was 2-3)

**Slip clause**: If Phase 3 exceeds 4 days, cut K2 (OTel exporter) to v0.5; ship K1 (tracing + request_id) alone.

- `tracing` + `tracing-subscriber` wiring; request_id UUID v7 via request-scoped span.
- Audit log schema v2 — `request_id` added as **last field** of `AuditEntry`. Non-required for v1 readers.
- OTel exporter behind `otel` cargo feature; `--otlp-endpoint` CLI flag.
- `--check-request --cost` — estimated API calls + rate-limit window.
- Error-hint registry +6 entries (E10 hint included).
- **+20 tests**.

### Phase 4 — Safety / cache / storage (3-4 days, was 2-3)

- `Cache` trait + LRU default (`src/cache/`). Invalidation on write to same entity.
- Idempotency-Key auto-gen + MCP override.
- `TokenProvider` trait in **`src/token_provider/`** (NOT `src/auth/` — M1 rename). Backends: `env`, `file`, `keychain` (macOS-gated), `exec`. Shadowing warning per E6.
- MCP tool JSON example injection via `#[doc = include_str!(...)]` (E11 pattern).
- `Cargo.toml` platform-gated dep: `[target.'cfg(target_os = "macos")'.dependencies] security-framework = "..."`.
- **+18 tests**.

### Integration wrap

- Live smoke test (BlueNode workspace; includes `db update --to-workspace` 403 path).
- Doc sweep + README.
- Release pipeline.

---

## Quality gates

- 280 + **~81 = ~360 tests**, zero regressions.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo audit` clean.
- **MCP snapshot: RO 7 / write 13 / admin 18** — enforced via `tests/mcp_server.rs`.
- Each new tool: wire-format snapshot + wiremock round-trip + rejection path.
- `tests/cookbook_examples.rs` passes (E7 rot prevention).
- D9/D10 maintained — `users_list`/`users_get`/`comments list` stay CLI-only. Exception: `users_me` at RO (§D9-ex).
- Justin 11-principle compliance matrix → 11/11 A-grade (see below).

---

## Justin 11-principle matrix

| # | Principle | v0.3 | v0.4 target |
|---|-----------|------|-------------|
| 1 | Universal `--json` | B | **A** (C1, M6) |
| 2 | `--output` = path | A | A |
| 3 | Field masks | B | **A-** (API limit) (C7, C8) |
| 4 | Streaming / paginated | B- | **A** (C2, E4) |
| 5 | Structured errors | A | A (K1 adds request_id) |
| 6 | Dry-run semantics | A- | **A** (C3, C4) |
| 7 | Verbose by default | A | A (never `--quiet`) |
| 8 | Explicit rules | B+ | **A** (S1, E11) |
| 9 | Output envelope | A | A (E1 request_id) |
| 10 | Env-overridable | A- | **A** (K3) |
| 11 | Consistent terminology | A- | **A** (E9) |

Baseline corrected: Principle #3 was B in v0.3 (not A; Justin baseline re-verified per architect M5).

---

## Repo changes summary

### Adds

```
docs/
├── cookbook/                    ← NEW canonical (E7)
│   ├── bootstrap-workspace.md
│   ├── bulk-import-csv.md
│   ├── reconcile-schema.md
│   ├── agent-idempotent-writes.md
│   └── snippets/                ← JSON examples used by #[doc = include_str!]
│       ├── db_create.md
│       ├── db_update.md
│       ├── ds_update.md
│       ├── ds_add_relation.md
│       ├── page_move.md
│       └── users_me.md
└── runtime-samples/             ← NEW sample only
    ├── hermes-profile.sample.yaml
    ├── claude-desktop.sample.json
    └── cursor-mcp.sample.json

src/
├── observability/               ← NEW
│   ├── mod.rs
│   ├── request_id.rs
│   ├── tracing.rs
│   └── otel.rs (cfg otel)
├── cache/                       ← NEW
│   ├── mod.rs
│   └── lru.rs
└── token_provider/              ← NEW (M1 rename from src/auth/)
    ├── mod.rs
    └── providers/
        ├── env.rs
        ├── file.rs
        ├── keychain.rs (cfg target_os = "macos")
        └── exec.rs

tests/
├── cache.rs                     ← NEW
├── observability.rs             ← NEW
├── stream_jsonl.rs              ← NEW
├── json_body.rs                 ← NEW
└── cookbook_examples.rs         ← NEW (S6 rot gate)
```

### Changes

- `Cargo.toml` 0.3.0 → 0.4.0. Deps: `tracing = "0.1"`, `tracing-subscriber = "0.3"`, `uuid = { version = "1", features = ["v4", "v7"] }`, `lru = "0.16"`. Feature `otel` → (`opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry`). **Platform dep**: `[target.'cfg(target_os = "macos")'.dependencies] security-framework = "3"`.
- `clawhub/SKILL.md` → v2.1.0.
- Mutation CLI subcommands gain `--json` / `--check-request` / `--dry-run`.
- MCP write/admin tool structs gain optional `json` field.
- `tests/mcp_server.rs` snapshot expectations updated (Phase 1 first commit).

### Preserved (BREAKING = zero for JSON consumers)

- Every v0.3 CLI subcommand / MCP tool / env var signature.
- `--check-request` / `--raw` / `--pretty` semantics.
- Audit JSONL remains parseable by v0.3 JSON-key readers. Positional parsers see new `request_id` field appended.

---

## Release pipeline

1. `Cargo.toml`: 0.3.0 → 0.4.0.
2. `CHANGELOG.md` v0.4.0 section — non-BREAKING note + positional-parser caveat.
3. `README.md`: 360 tests, 7/12/17 tools, Justin 11/11 A-grade.
4. Tag `v0.4.0` → cargo-dist 4-platform build + GitHub release.
5. `cargo publish notion-cli-mcp 0.4.0`.
6. `clawhub publish ./clawhub --version 2.1.0`.
7. Scanner verify: v2.1.0 expected CLEAN (envs unchanged, `[NOTION_TOKEN]` only).

---

## TL;DR

**v0.4 = Justin agent-first 11-principle full alignment + missing API trio (db update / users me / page get-property) + universal `--json` + observability baseline (request_id / tracing / OTel feature) + cache + idempotency + TokenProvider. 12-14 days, non-BREAKING minor.**

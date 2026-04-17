# notion-cli

[![CI](https://img.shields.io/badge/tests-130_passed-brightgreen)]()
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)]()

An agent-first Notion CLI and MCP server, purpose-built for the Notion API 2025-09-03+ data-source model.

## Features

- **12 tools** — query, search, get/create/update pages, get/create data sources, full block CRUD (retrieve, list children, append, update, delete)
- **Page body support** — `page create --children` for one-shot page + body creation
- **CLI + MCP** — same tool surface accessible via shell commands or as an [MCP](https://modelcontextprotocol.io/) stdio server
- **Agent-friendly output** — responses wrapped in an untrusted-source envelope with trust metadata; `--raw` for clean piping
- **Rate limiting** — built-in 3 req/s token bucket with 429 `Retry-After` retry (configurable)
- **Response size cap** — 10 MiB streaming limit prevents OOM on large payloads
- **Schema introspection** — `notion-cli schema <type>` emits JSON Schema for 22 property value types, 12 block types, and more
- **Read-only MCP default** — write tools require explicit `--allow-write`; write operations logged to JSONL audit trail
- **Actionable error hints** — common Notion validation errors get one-line remediation suggestions
- **Structured exit codes** — stable numeric codes (0/2/3/4/10/64/65/74) for scripting and CI
- **Newtype ID validation** — accepts 32-hex, dashed UUID, or full Notion URLs; rejects homoglyphs and control characters
- **`--check-request`** — validate and preview request payloads locally without contacting Notion

## Quick Start

```sh
# Install
cargo install --git https://github.com/0xarkstar/notion-cli

# Set your integration token
export NOTION_TOKEN='ntn_...'

# Search across your workspace
notion-cli search 'meeting notes'

# Query a data source with filters
notion-cli ds query <data-source-id> \
  --filter '{"property":"Done","checkbox":{"equals":false}}'
```

## Installation

### crates.io

```sh
cargo install notion-cli
```

### Prebuilt binaries (macOS / Linux, aarch64 + x86_64)

```sh
curl -LsSf https://github.com/0xarkstar/notion-cli/releases/latest/download/notion-cli-installer.sh | sh
```

### Homebrew

```sh
brew install 0xarkstar/tap/notion-cli
```

### From source

```sh
cargo install --git https://github.com/0xarkstar/notion-cli
```

Requires Rust 1.85+. Run `notion-cli --version` to verify.

## Usage

### Pages

```sh
# Retrieve a page (accepts IDs or full Notion URLs)
notion-cli page get https://notion.so/My-Page-abcdef0123456789abcdef0123456789

# Create a page under a data source
notion-cli page create \
  --parent-data-source <data-source-id> \
  --properties '{
    "Name": {"type":"title","title":[{"type":"text","text":{"content":"New page"}}]},
    "Status": {"type":"status","status":{"name":"In Progress"}}
  }'

# Update properties
notion-cli page update <page-id> \
  --properties '{"Status":{"type":"status","status":{"name":"Done"}}}'

# Archive
notion-cli page archive <page-id>
```

### Data Sources

```sh
# Retrieve schema and metadata
notion-cli ds get <data-source-id>

# Query with filter, sort, and pagination
notion-cli ds query <data-source-id> \
  --filter '{"property":"Priority","select":{"equals":"High"}}' \
  --sorts '[{"property":"Due","direction":"ascending"}]' \
  --page-size 25

# Create a new data source in a database container
notion-cli ds create \
  --parent <database-id> \
  --title 'Tasks' \
  --properties '{"Name":{"title":{}},"Done":{"checkbox":{}}}'
```

### Search & Introspection

```sh
# Full-text search (filter by object type)
notion-cli search 'onboarding' \
  --filter '{"property":"object","value":"page"}'

# Retrieve a database container (lists its data sources)
notion-cli db get <database-id>

# Print JSON Schema for property values (all 22 types)
notion-cli schema property-value --pretty
```

### Request Validation

Preview what would be sent without making an API call:

```sh
notion-cli --check-request --pretty page create \
  --parent-data-source <id> \
  --properties '{"Name":{"type":"title","title":[{"type":"text","text":{"content":"test"}}]}}'
```

No token required for `--check-request`.

## MCP Server

Start as an [MCP](https://modelcontextprotocol.io/) stdio server for agent integration:

```sh
# Read-only (4 tools: get_page, get_data_source, query_data_source, search)
notion-cli mcp

# Full access (7 tools: above + create_page, update_page, create_data_source)
notion-cli mcp --allow-write

# With audit logging
notion-cli mcp --allow-write --audit-log ./notion-audit.jsonl
```

### Agent configuration example

```yaml
mcp_servers:
  notion:
    command: notion-cli
    args: [mcp, --allow-write, --audit-log, /var/log/notion-audit.jsonl]
    env:
      NOTION_TOKEN: ntn_xxx
    enabled: true
```

## Output Format

Responses are wrapped in a trust-demarcation envelope by default:

```json
{
  "source": "notion",
  "trust": "untrusted",
  "api_version": "2026-03-11",
  "content": { ... }
}
```

This signals to consuming agents that `content` originates from an external source and should be treated as data, not instructions. Use `--raw` to strip the envelope, and `--pretty` for indented output.

## Configuration

| Setting | Source | Description |
|---------|--------|-------------|
| `NOTION_TOKEN` | env / `--token` | Integration token (`ntn_...`) |
| `NOTION_CLI_AUDIT_LOG` | env / `--audit-log` | Path for write-operation JSONL audit log |

Create an integration at [notion.so/my-integrations](https://www.notion.so/my-integrations), then share target pages/databases with it via the **Connections** menu.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 2 | Validation error |
| 3 | API error |
| 4 | Rate-limited |
| 10 | Auth / config error |
| 64 | Usage error |
| 65 | JSON parse error |
| 74 | I/O error |

## Development

```sh
cargo test              # 130 tests
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --all-targets --summary-only   # 80%+ line coverage
cargo audit             # 0 advisories

# Live smoke test against a real workspace
export NOTION_TOKEN='ntn_...'
cargo run --example smoke -- <database-url>
```

## Roadmap

- Block CRUD (page body / children)
- NDJSON streaming for paginated queries
- `--fields` response field masks
- YAML configuration file
- Multi-platform binaries via cargo-dist / Homebrew

## License

[MIT](LICENSE)

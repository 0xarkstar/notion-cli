# notion-cli: Agent-First Notion CLI + MCP Tool

## Overview

A Rust CLI tool for Notion that follows Agent-First design principles. Built for AI agents (Hermes, Claude Code) to interact with Notion workspaces via CLI and MCP protocol.

## Decision Record

**Language: Rust** — decided after 6-person team debate (Go 3 vs Rust 3).

### Why Rust

The core requirement of an Agent-First CLI is **schema-implementation consistency**. When an AI agent reads a tool's JSON Schema and constructs a request, the schema MUST match the implementation. Schema drift = agent sends malformed requests = wasted tokens + failed operations.

Rust eliminates schema drift structurally:
- `#[derive(Deserialize, JsonSchema)]` on a single parameter struct generates both the deserialization logic AND the JSON Schema from one source of truth
- `#[serde(tag = "type")]` enum models Notion's 22 property types with compile-time exhaustiveness
- `#[serde(other)] Unknown` provides graceful degradation for new types while maintaining compile-time safety for known types
- Newtype pattern (`struct DatabaseId(String)`) prevents ID confusion at compile time
- rmcp `#[tool]` macro auto-generates MCP tool definitions from function signatures

### Why Not Go

Go was the strong alternative (user has `remops` — 108 files, 17k lines, Go CLI+MCP+HTTP). Go advantages acknowledged:
- Faster compilation (1-2s vs 4-10s)
- goreleaser maturity (15k★)
- mcp-go ecosystem (8.5k★)
- Proven multi-surface architecture in remops

Go's fatal weakness: schema and implementation are separate artifacts. The MCP builder (`mcp.NewTool()` + `WithString()`) and the handler's parameter struct must be kept in sync manually. Every field addition requires two changes in two places. This is the dominant maintenance burden and the primary failure mode for agent tooling.

### Key References

- [Rewrite your CLI for AI Agents](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/) — Agent-First CLI design principles
- [Google Code Syntax Guidelines](https://developers.google.com/style/code-syntax)
- NousResearch/hermes-agent#7449 — Our Gemma 4 parser PR (context for understanding tool call parsing)

## Architecture

```
notion-cli (Rust)
├── src/
│   ├── main.rs              # clap CLI entry point
│   ├── cli/                  # CLI commands (clap derive)
│   │   ├── mod.rs
│   │   ├── database.rs       # db query, create, update
│   │   ├── page.rs           # page create, get, update, archive
│   │   ├── search.rs         # search pages and databases
│   │   ├── block.rs          # block CRUD
│   │   └── schema.rs         # schema introspection subcommand
│   ├── api/                  # Notion REST API client
│   │   ├── mod.rs
│   │   ├── client.rs         # reqwest HTTP client with auth
│   │   ├── database.rs       # POST /v1/databases, /v1/databases/{id}/query
│   │   ├── page.rs           # POST /v1/pages, PATCH /v1/pages/{id}
│   │   ├── block.rs          # GET/PATCH /v1/blocks/{id}/children
│   │   └── search.rs         # POST /v1/search
│   ├── types/                # Notion API types (serde + schemars)
│   │   ├── mod.rs
│   │   ├── property.rs       # 22 PropertyValue variants (tagged enum)
│   │   ├── block.rs          # 26+ BlockType variants
│   │   ├── filter.rs         # Filter expression types
│   │   ├── sort.rs           # Sort criteria
│   │   ├── rich_text.rs      # RichText with annotations
│   │   ├── page.rs           # Page object
│   │   ├── database.rs       # Database object
│   │   └── common.rs         # Shared types (Color, Icon, Cover, etc.)
│   ├── mcp/                  # MCP server (rmcp)
│   │   ├── mod.rs
│   │   ├── server.rs         # #[tool_router] impl
│   │   └── tools.rs          # Tool parameter structs
│   ├── output/               # Output formatting
│   │   ├── mod.rs
│   │   ├── json.rs           # JSON output (agent-first default)
│   │   ├── table.rs          # Table output (human-friendly)
│   │   └── ndjson.rs         # NDJSON streaming for pagination
│   ├── validation/           # Adversarial input validation
│   │   ├── mod.rs
│   │   ├── ids.rs            # NotionId, DatabaseId, PageId, BlockId newtypes
│   │   └── sanitize.rs       # Output sanitization (LLM injection prevention)
│   └── config.rs             # YAML config + env vars
├── tests/                    # Integration tests
│   ├── property_roundtrip.rs # proptest: 22 property types serialize/deserialize
│   ├── mcp_tools.rs          # MCP tool schema validation
│   └── cli_output.rs         # CLI JSON output format verification
├── skills/                   # SKILL.md files for agent guidance
│   └── notion-query/
│       └── SKILL.md
├── Cargo.toml
├── DESIGN.md                 # This file
└── README.md
```

## Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.6+ | CLI framework (derive macros) |
| `serde` + `serde_json` | 1.x | JSON serialization/deserialization |
| `schemars` | 1.2+ | JSON Schema generation from types |
| `rmcp` | 1.4+ | MCP server (official Rust SDK) |
| `reqwest` | 0.12+ | HTTP client for Notion API |
| `tokio` | 1.x | Async runtime |
| `thiserror` | 2.x | Error types |
| `proptest` | 1.x | Property-based testing |

## Agent-First Design Principles

Based on the Poehnelt article, implemented in Rust:

### 1. Raw JSON Payloads Over Custom Flags

```rust
#[derive(Parser)]
struct CreatePageCmd {
    #[arg(long)]
    database_id: DatabaseId,
    /// Raw JSON properties (Agent-First: accepts full API payload)
    #[arg(long, value_parser = parse_properties)]
    json: Option<String>,
}
```

### 2. Runtime Schema Introspection

```rust
// notion-cli schema property-value
// → outputs JSON Schema auto-generated from PropertyValue enum
fn handle_schema(resource: SchemaResource) {
    let schema = match resource {
        SchemaResource::PropertyValue => schemars::schema_for!(PropertyValue),
        SchemaResource::BlockType => schemars::schema_for!(BlockType),
        SchemaResource::Filter => schemars::schema_for!(FilterExpression),
    };
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
```

### 3. Context Window Discipline

- `--fields` flag for field masks (reduce response size)
- NDJSON pagination (`--output ndjson`) for incremental processing
- Compact JSON by default, `--pretty` for human reading

### 4. Adversarial Input Validation

```rust
pub struct DatabaseId(String); // private field = only constructible via validate()

impl DatabaseId {
    pub fn validate(input: &str) -> Result<Self, ValidationError> {
        let decoded = percent_decode_str(input).decode_utf8()?;
        if decoded.contains("..") || decoded.chars().any(|c| c < '\x20') {
            return Err(ValidationError::SuspiciousInput);
        }
        let normalized = decoded.replace('-', "");
        if normalized.len() != 32 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidId);
        }
        Ok(Self(normalized))
    }
}
```

### 5. Multi-Surface Exposure

```
notion-cli query <db-id>              # CLI (human + agent)
notion-cli mcp                        # MCP stdio server (agent)
NOTION_TOKEN=xxx notion-cli ...       # Env var auth (headless)
```

### 6. Safety Rails

- `--dry-run` validates request without API call
- Output sanitization strips LLM injection patterns

## Notion API Type Modeling

The killer feature — Notion's 22 property types as a Rust enum:

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PropertyValue {
    Title { title: Vec<RichText> },
    RichText { rich_text: Vec<RichText> },
    Number { number: Option<f64> },
    Select { select: Option<SelectOption> },
    MultiSelect { multi_select: Vec<SelectOption> },
    Date { date: Option<DateValue> },
    People { people: Vec<User> },
    Files { files: Vec<FileObject> },
    Checkbox { checkbox: bool },
    Url { url: Option<String> },
    Email { email: Option<String> },
    PhoneNumber { phone_number: Option<String> },
    Formula { formula: FormulaValue },
    Relation { relation: Vec<RelationRef> },
    Rollup { rollup: RollupValue },
    CreatedTime { created_time: String },
    CreatedBy { created_by: User },
    LastEditedTime { last_edited_time: String },
    LastEditedBy { last_edited_by: User },
    Status { status: Option<StatusOption> },
    UniqueId { unique_id: UniqueIdValue },
    Verification { verification: VerificationValue },
    #[serde(other)]
    Unknown,
}
```

One `#[derive]` line = serialization + deserialization + JSON Schema + compile-time exhaustiveness + graceful unknown handling.

## MCP Tool Pattern

```rust
#[derive(Deserialize, JsonSchema)]
struct QueryDatabaseParams {
    /// Database UUID
    database_id: DatabaseId,
    /// Notion filter expression
    #[serde(default)]
    filter: Option<FilterExpression>,
    /// Sort criteria
    #[serde(default)]
    sorts: Vec<SortCriterion>,
    /// Results per page (1-100)
    #[schemars(range(min = 1, max = 100))]
    #[serde(default = "default_page_size")]
    page_size: u8,
}

#[tool_router]
impl NotionTools {
    #[tool(description = "Query a Notion database with filters and sorts")]
    async fn query_database(
        &self,
        #[tool(param)] params: QueryDatabaseParams,
    ) -> String {
        // Schema auto-generated from QueryDatabaseParams
        // Parameter validation auto-handled by serde
        // DatabaseId validated by newtype constructor
    }
}
```

## CLI Commands (Planned)

| Command | Description |
|---------|-------------|
| `notion db list` | List databases |
| `notion db query <id>` | Query database with --filter, --sort |
| `notion db create` | Create database |
| `notion page get <id>` | Get page |
| `notion page create <db-id>` | Create page in database |
| `notion page update <id>` | Update page properties |
| `notion page archive <id>` | Archive page |
| `notion search <query>` | Search pages and databases |
| `notion block list <id>` | List block children |
| `notion block append <id>` | Append blocks |
| `notion schema <type>` | Introspect JSON Schema for a type |
| `notion mcp` | Start MCP stdio server |

## Configuration

```yaml
# ~/.config/notion-cli/config.yaml
notion_token: ${NOTION_TOKEN}  # env var interpolation
default_database: "abc123"     # alias for frequently used DB
output:
  format: json                 # json | table | ndjson
  pretty: false
  fields: []                   # default field mask (empty = all)
```

## Testing Strategy

1. **Property roundtrip tests** (proptest): auto-generate all 22 PropertyValue variants, serialize to JSON, deserialize back, verify equality
2. **MCP schema validation**: verify generated schemas match expected JSON Schema structure
3. **CLI output tests**: snapshot testing for JSON output format consistency
4. **Adversarial input tests**: path traversal, double encoding, control chars, Unicode edge cases
5. **Integration tests**: real Notion API calls against a test workspace (CI only)

## Distribution

- `cargo-dist` for multi-platform binaries (macOS ARM/x86, Linux ARM/x86)
- GitHub Releases with checksums
- Homebrew tap (via cargo-dist)
- Single static binary, no runtime dependencies

## Related Projects

- `remops` (Go) — Same user's Docker management CLI+MCP+HTTP. Architecture patterns (multi-surface, security pipeline, output formatting) informed this design.
- `RTK` (Rust) — Same user's Rust CLI proxy. Rust toolchain experience.
- NousResearch/hermes-agent#7449 — Gemma 4 tool call parser PR by same user.

## Integration with BlueNode Agent

This CLI will be connected to the BlueNode Discord agent (Hermes) as an MCP server, replacing the broken `@notionhq/notion-mcp-server` which has an API version incompatibility (`create_a_data_source` fails with validation_error on API version 2025-09-03+).

```yaml
# ~/.hermes/profiles/bluenode/config.yaml
mcp_servers:
  notion:
    command: notion-cli
    args: [mcp]
    env:
      NOTION_TOKEN: "ntn_xxx"
    enabled: true
```

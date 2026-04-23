# Bulk Import from CSV

Iterate rows from a CSV file and create a Notion page per row using
`notion-cli page create`. Requires `--allow-write` on the MCP server
or a shell with `NOTION_TOKEN` set.

## Shell script

```bash
#!/usr/bin/env bash
# Usage: ./import-csv.sh <data-source-id> <csv-file>
# CSV format: Name,Status,Due
# Example:    Launch v0.4,In Progress,2026-05-01

DS_ID="$1"
CSV="$2"

if [[ -z "$DS_ID" || -z "$CSV" ]]; then
  echo "Usage: $0 <data-source-id> <csv-file>" >&2
  exit 1
fi

tail -n +2 "$CSV" | while IFS=',' read -r name status due; do
  props=$(jq -n \
    --arg name "$name" \
    --arg status "$status" \
    --arg due "$due" \
    '{
      "Name":   {"type":"title","title":[{"type":"text","text":{"content":$name}}]},
      "Status": {"type":"status","status":{"name":$status}},
      "Due":    {"type":"date","date":{"start":$due}}
    }')
  notion-cli page create \
    --parent-data-source "$DS_ID" \
    --properties "$props"
  echo "Created: $name"
done
```

## Dry-run first

Before importing, validate that the property shape is correct:

```bash
notion-cli --check-request --pretty page create \
  --parent-data-source "$DS_ID" \
  --properties "$props"
```

## Notes

- The script is intentionally single-threaded to respect Notion's rate
  limits (3 req/s for write ops on most integration tiers).
- `notion-cli` has a built-in token-bucket rate limiter; it will
  automatically back off on 429 responses with `Retry-After`.
- For large imports (1000+ rows), consider batching with `--children`
  to set page body content in the same API call as creation.

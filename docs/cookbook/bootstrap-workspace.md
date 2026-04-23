# Bootstrap a Workspace: End-to-End

Create a parent page, a database container with two data sources, add
a relation between them, then seed rows — using both the CLI and MCP
tool forms.

## CLI walkthrough

```bash
# 1. Create a parent page to hold the database
notion-cli page create \
  --parent-page <workspace-root-page-id> \
  --properties '{"title":{"type":"title","title":[{"type":"text","text":{"content":"Project Hub"}}]}}'

# 2. Create the database container (returns container id + initial DS id)
notion-cli db create \
  --parent-page <parent-page-id> \
  --title "Tasks" \
  --icon 📋 \
  --schema ./schemas/tasks.json

# schemas/tasks.json:
# {
#   "Name":     {"type":"title","title":{}},
#   "Status":   {"type":"status","status":{"options":[]}},
#   "Due":      {"type":"date","date":{}}
# }

# 3. Inspect the database to get the data source ID
notion-cli --raw db get <database-id> | jq -r '.data_sources[0].id'

# 4. Create a second data source inside the same container
notion-cli ds create \
  --parent <database-id> \
  --title "Milestones" \
  --properties '{"Name":{"type":"title","title":{}},"Due":{"type":"date","date":{}}}'

# 5. Wire a relation from Tasks DS → Milestones DS
notion-cli ds add-relation <tasks-ds-id> \
  --name "Milestone" --target <milestones-ds-id> --backlink "Tasks"

# 6. Create a page in the Tasks data source
notion-cli page create \
  --parent-data-source <tasks-ds-id> \
  --properties '{
    "Name":{"type":"title","title":[{"type":"text","text":{"content":"Launch v0.4"}}]},
    "Status":{"type":"status","status":{"name":"In Progress"}}
  }'
```

## MCP equivalent (agent call sequence)

```json
// Step 1: create parent page
{"tool":"create_page","params":{"parent_page_id":"<root>","properties":{...}}}

// Step 2: create database (admin tier required)
{"tool":"db_create","params":{"parent_page_id":"<parent>","title":"Tasks","properties":{...}}}

// Step 3: query DB to get DS id
{"tool":"get_data_source","params":{"data_source_id":"<ds-id>"}}

// Step 4: update DB metadata (v0.4)
{"tool":"db_update","params":{"database_id":"<db-id>","title":"Tasks v2"}}

// Step 5: relation wiring (admin tier)
{"tool":"ds_add_relation","params":{"source_data_source_id":"<tasks-ds>","name":"Milestone","target_data_source_id":"<milestones-ds>","backlink":"Tasks"}}

// Step 6: seed a row
{"tool":"create_page","params":{"parent_data_source_id":"<tasks-ds>","properties":{...}}}
```

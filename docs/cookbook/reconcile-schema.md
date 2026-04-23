# Reconcile Schema: Diff and Converge

Use `db get` and `ds update` to compare the live Notion schema against
a desired-state schema file and apply the delta.

## Inspect live schema

```bash
# Get the database container (includes data_sources array)
notion-cli --raw db get <database-id> | jq '.data_sources[0].id'

# Get the data source schema (property map)
notion-cli --raw ds get <data-source-id> | jq '.properties | keys'
```

## Compare to desired state

```bash
# desired-schema.json contains the target property map
LIVE=$(notion-cli --raw ds get <data-source-id> | jq '[.properties | keys[]]' | sort)
WANT=$(jq '[keys[]]' desired-schema.json | sort)

# Properties to add
comm -13 <(echo "$LIVE" | tr -d '[],"' | sort) \
         <(echo "$WANT" | tr -d '[],"' | sort)

# Properties to remove (review carefully before removing)
comm -23 <(echo "$LIVE" | tr -d '[],"' | sort) \
         <(echo "$WANT" | tr -d '[],"' | sort)
```

## Apply delta (single-delta per invocation)

```bash
# Add a missing property
notion-cli ds update add-property <ds-id> \
  --name "Priority" \
  --schema '{"type":"select","select":{"options":[{"name":"High"},{"name":"Low"}]}}'

# Rename a property
notion-cli ds update rename-property <ds-id> \
  --from "OldName" --to "NewName"

# Remove a property (destructive — requires --yes on non-TTY)
notion-cli ds update remove-property <ds-id> --name "DeprecatedField" --yes
```

## MCP equivalent (agent, admin tier)

```json
// Add property
{"tool":"ds_update","params":{"data_source_id":"<id>","action":"add_property","name":"Priority","schema":{"type":"select","select":{"options":[]}}}}

// Rename property
{"tool":"ds_update","params":{"data_source_id":"<id>","action":"rename_property","name":"OldName","new_name":"NewName"}}

// Remove property (requires confirm=true + NOTION_CLI_ADMIN_CONFIRMED=1 env)
{"tool":"ds_update","params":{"data_source_id":"<id>","action":"remove_property","name":"DeprecatedField","confirm":true}}
```

## Update database container metadata (v0.4)

```bash
# Rename the database container itself
notion-cli db update <database-id> --title "Tasks v2"

# Move the database to a new parent page
notion-cli db update <database-id> --to-page <new-parent-page-id>

# Clear the icon
notion-cli db update <database-id> --icon-clear
```

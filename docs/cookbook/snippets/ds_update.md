# `ds_update` — example JSON per action

## `add_property`

```json
{
  "data_source_id": "fedcba9876543210fedcba9876543210",
  "action": "add_property",
  "name": "Priority",
  "schema": {
    "type": "select",
    "select": {"options": [{"name": "High"}, {"name": "Low"}]}
  }
}
```

## `remove_property` (destructive)

Requires `confirm: true` AND `NOTION_CLI_ADMIN_CONFIRMED=1` env on
the `notion-cli mcp` process.

```json
{
  "data_source_id": "fedcba9876543210fedcba9876543210",
  "action": "remove_property",
  "name": "DeprecatedField",
  "confirm": true
}
```

## `rename_property`

```json
{
  "data_source_id": "fedcba9876543210fedcba9876543210",
  "action": "rename_property",
  "name": "OldName",
  "new_name": "NewName"
}
```

## `add_option`

```json
{
  "data_source_id": "fedcba9876543210fedcba9876543210",
  "action": "add_option",
  "property": "Priority",
  "kind": "select",
  "option": {"name": "Urgent", "color": "red"}
}
```

## bulk (non-atomic, partial failure possible)

```json
{
  "data_source_id": "fedcba9876543210fedcba9876543210",
  "action": "bulk",
  "body": {
    "title": [{"type":"text","text":{"content":"Renamed DS"}}],
    "properties": {
      "NewProp": {"type": "checkbox", "checkbox": {}},
      "OldProp": null
    }
  }
}
```

# `page_move` — example JSON

Uses `POST /v1/pages/{id}/move` — the dedicated endpoint introduced
2026-01-15. `PATCH /v1/pages/{id}` explicitly rejects parent mutation.

## Move to a page

```json
{
  "page_id": "11111111111111111111111111111111",
  "target_page_id": "22222222222222222222222222222222"
}
```

## Move to a data source (into a database row)

```json
{
  "page_id": "11111111111111111111111111111111",
  "target_data_source_id": "fedcba9876543210fedcba9876543210"
}
```

`target_page_id` and `target_data_source_id` are mutually exclusive.
Restrictions: source must be a regular page (not a database);
integration must have edit access to the new parent;
cross-workspace moves are server-rejected.

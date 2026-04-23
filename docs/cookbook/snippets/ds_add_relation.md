# `ds_add_relation` — example JSON

## Two-way (backlink)

Creates a relation property on `source` and a reciprocal property
on `target` with the name given in `backlink`.

```json
{
  "source_data_source_id": "aaaabbbbccccddddaaaabbbbccccdddd",
  "name": "Milestone",
  "target_data_source_id": "11112222333344441111222233334444",
  "backlink": "Tasks"
}
```

## One-way (no backlink)

```json
{
  "source_data_source_id": "aaaabbbbccccddddaaaabbbbccccdddd",
  "name": "Related",
  "target_data_source_id": "11112222333344441111222233334444",
  "one_way": true
}
```

## Self-referential

Source and target are the same data source. Skips the pre-flight GET
on the target.

```json
{
  "source_data_source_id": "aaaabbbbccccddddaaaabbbbccccdddd",
  "name": "ParentTask",
  "self": true
}
```

Note: exactly one of `backlink`, `one_way`, or `self` is required.

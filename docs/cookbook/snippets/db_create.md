# `db_create` — example JSON

```json
{
  "parent_page_id": "abcdef0123456789abcdef0123456789",
  "title": "Task Tracker",
  "icon": "📋",
  "is_inline": false,
  "properties": {
    "Name":     {"type": "title",  "title": {}},
    "Status":   {"type": "status", "status": {"options": []}},
    "Due":      {"type": "date",   "date": {}},
    "Priority": {
      "type": "select",
      "select": {
        "options": [
          {"name": "High",   "color": "red"},
          {"name": "Medium", "color": "yellow"},
          {"name": "Low",    "color": "green"}
        ]
      }
    }
  }
}
```

`properties` must contain at least one `title`-typed entry. The `icon`
field accepts an emoji literal or an `https://` URL.

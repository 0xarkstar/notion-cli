# `db_update` — example JSON

## Title-only update

```json
{
  "database_id": "abcdef0123456789abcdef0123456789",
  "title": "Task Tracker v2"
}
```

## Move to a new parent page

```json
{
  "database_id": "abcdef0123456789abcdef0123456789",
  "to_page_id": "11111111111111111111111111111111"
}
```

`to_page_id` and `to_workspace` are mutually exclusive.

## Clear the icon (tristate null)

```json
{
  "database_id": "abcdef0123456789abcdef0123456789",
  "icon": null
}
```

`icon: null` clears the icon. Omit the field entirely to leave it
unchanged. Pass a string value (`"🚀"` or `"https://..."`) to set it.

## Set cover + lock

```json
{
  "database_id": "abcdef0123456789abcdef0123456789",
  "cover": "https://images.example.com/cover.jpg",
  "is_locked": true
}
```

# `users_me` — example

No parameters required.

```json
{}
```

## Response shape

```json
{
  "source": "notion",
  "trust": "untrusted",
  "api_version": "2026-03-11",
  "content": {
    "object": "user",
    "id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    "type": "bot",
    "bot": {
      "owner": {
        "type": "workspace",
        "workspace": true
      },
      "workspace_name": "My Workspace"
    },
    "name": "My Integration",
    "avatar_url": null
  }
}
```

The response is the integration bot user associated with the token.
It does NOT enumerate other workspace users — only the caller's own
identity is returned. This makes it safe to expose in all MCP tiers
(D9 exception).

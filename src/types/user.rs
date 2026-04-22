//! Notion user object — person + bot variants.
//!
//! Wire format (API 2026-03-11):
//!
//! ```json
//! {"object":"user","id":"<uuid>","type":"person",
//!  "person":{"email":"a@b.com"},"name":"Foo","avatar_url":"..."}
//!
//! {"object":"user","id":"<uuid>","type":"bot",
//!  "bot":{"owner":{...},"workspace_name":"..."},
//!  "name":"My Integration"}
//! ```
//!
//! `users list` returns an array of these; `users get <id>` returns one.
//! Fields beyond `id`/`type` may be absent in partial-access responses
//! — they're all `Option<_>` / `#[serde(default)]`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validation::UserId;

/// A Notion user. Discriminated by the `type` field on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: UserId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(flatten, default)]
    pub kind: Option<UserKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserKind {
    Person { person: PersonFields },
    Bot {
        #[serde(default)]
        bot: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct PersonFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl User {
    #[must_use]
    pub fn is_bot(&self) -> bool {
        matches!(self.kind, Some(UserKind::Bot { .. }))
    }

    #[must_use]
    pub fn is_person(&self) -> bool {
        matches!(self.kind, Some(UserKind::Person { .. }))
    }
}

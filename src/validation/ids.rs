//! Notion ID newtypes.
//!
//! Two constructors:
//! - [`parse`][DatabaseId::parse]: strict format — 32 hex characters,
//!   dashes optional, rejects URLs.
//! - [`from_url_or_id`][DatabaseId::from_url_or_id]: extracts a 32-hex ID
//!   from a Notion URL, or parses a raw ID.
//!
//! Design notes:
//! - No percent-decoding. Notion IDs are not URL-encoded at this layer;
//!   decoding would silently rewrite input and violate fail-closed.
//! - No path-traversal check. IDs must never be used as filesystem paths;
//!   if caching is added, path safety lives at the FS boundary via hashing.
//! - Normalisation is lowercase, no dashes. `as_dashed()` renders the
//!   8-4-4-4-12 UUID form when needed.

use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

macro_rules! define_notion_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Parse a strictly-formatted Notion ID.
            ///
            /// Accepts 32 hex characters, optionally dashed as the
            /// 8-4-4-4-12 UUID layout. Rejects URLs — use
            /// [`Self::from_url_or_id`] to accept either.
            pub fn parse(input: &str) -> Result<Self> {
                let hex: String = input.chars().filter(|c| *c != '-').collect();
                if hex.len() != 32 {
                    return Err(Error::invalid_id(
                        "must be 32 hex characters (dashes optional)",
                        input,
                    ));
                }
                if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(Error::invalid_id(
                        "must contain only hex digits and dashes",
                        input,
                    ));
                }
                Ok(Self(hex.to_ascii_lowercase()))
            }

            /// Parse either a raw ID or a Notion URL.
            ///
            /// For URLs: extracts the trailing 32-hex sequence after
            /// stripping query string and fragment.
            pub fn from_url_or_id(input: &str) -> Result<Self> {
                if input.contains("://") {
                    Self::from_url(input)
                } else {
                    Self::parse(input)
                }
            }

            fn from_url(input: &str) -> Result<Self> {
                let without_query = input.split('?').next().unwrap_or(input);
                let without_fragment = without_query.split('#').next().unwrap_or(without_query);
                let tail = without_fragment
                    .rsplit('/')
                    .next()
                    .ok_or_else(|| Error::InvalidUrl(input.to_string()))?;

                // Collect hex + dash chars from the right until non-matching.
                let hex_suffix: String = tail
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_hexdigit() || *c == '-')
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let dash_free: String = hex_suffix.chars().filter(|c| *c != '-').collect();
                if dash_free.len() < 32 {
                    return Err(Error::InvalidUrl(input.to_string()));
                }
                let start = dash_free.len() - 32;
                Self::parse(&dash_free[start..])
            }

            /// Return the normalised 32-hex form (lowercase, no dashes).
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Render with standard UUID dashes (8-4-4-4-12).
            pub fn as_dashed(&self) -> String {
                let s = &self.0;
                format!(
                    "{}-{}-{}-{}-{}",
                    &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32],
                )
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = Error;
            fn from_str(s: &str) -> Result<Self> {
                Self::from_url_or_id(s)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(
                d: D,
            ) -> std::result::Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Self::from_url_or_id(&s).map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(stringify!($name))
            }
            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                schemars::json_schema!({
                    "type": "string",
                    "description": concat!(
                        "Notion ",
                        stringify!($name),
                        " — 32 hex characters (optionally dashed as 8-4-4-4-12) or a Notion URL.",
                    ),
                })
            }
        }
    };
}

define_notion_id!(
    /// A Notion database container ID.
    ///
    /// Since API 2025-09-03 a database may have multiple data sources —
    /// see [`DataSourceId`].
    DatabaseId
);

define_notion_id!(
    /// A Notion data source ID. Introduced in API 2025-09-03.
    ///
    /// The broken `create_a_data_source` endpoint in
    /// `@notionhq/notion-mcp-server` is the reason this crate exists.
    DataSourceId
);

define_notion_id!(
    /// A Notion page ID.
    PageId
);

define_notion_id!(
    /// A Notion block ID.
    BlockId
);

define_notion_id!(
    /// A Notion user (person or bot) ID.
    UserId
);

//! Notion block types.
//!
//! # Design
//!
//! Same pattern as [`crate::types::property::Property`] to sidestep
//! [serde issue #912](https://github.com/serde-rs/serde/issues/912):
//!
//! - Outer [`Block`] is `#[serde(untagged)]` — tries [`Block::Known`]
//!   (a fully-typed [`TypedBlock`]), falls through to [`Block::Raw`]
//!   for forward-compatibility with block types this crate version
//!   doesn't model.
//! - Inner [`TypedBlock`] carries the common metadata fields (id,
//!   timestamps, `has_children`, etc.) and flattens in a
//!   [`BlockBody`] tagged enum for the type-specific content.
//!
//! # Writes
//!
//! When appending blocks as children, send only the [`BlockBody`]
//! variant — the metadata fields are set by Notion. Use
//! [`BlockBody`] directly (not [`Block`]) for create/append request
//! payloads; [`Block::Raw`] has no compatible wire format on writes
//! (unknown `type` discriminators are rejected).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::common::{Color, UserRef};
use crate::types::rich_text::RichText;
use crate::validation::BlockId;

// === Outer wrapper =========================================================

/// Graceful-degradation wrapper for Notion blocks.
///
/// Unknown block types fall through to [`Block::Raw`], preserving
/// the full JSON for read access. Write paths should use
/// [`BlockBody`] directly (see module docs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Block {
    /// Fully-typed block. Round-trip safe for the 12 modelled types.
    Known(Box<TypedBlock>),
    /// Fallback for block types not modelled by this crate version.
    /// Cannot be used in write operations.
    Raw(serde_json::Value),
}

impl Block {
    pub fn known(b: TypedBlock) -> Self {
        Self::Known(Box::new(b))
    }

    pub fn as_known(&self) -> Option<&TypedBlock> {
        match self {
            Self::Known(v) => Some(v),
            Self::Raw(_) => None,
        }
    }

    pub fn is_writable(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// A fully-typed Notion block with metadata + content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TypedBlock {
    pub id: BlockId,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub has_children: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub in_trash: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<UserRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_edited_by: Option<UserRef>,
    /// The type-specific content. Flattens so wire format matches
    /// `{id, ..., type: "paragraph", paragraph: {...}}`.
    #[serde(flatten)]
    pub body: BlockBody,
}

// === Block body (type-specific content) ===================================

/// The 12 modelled Notion block types, tagged by `type` on the wire.
///
/// This is what you send when appending block children. To receive a
/// block from the API, wrap in [`Block`] to get the `Raw` fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockBody {
    Paragraph {
        paragraph: TextBlock,
    },
    #[serde(rename = "heading_1")]
    Heading1 {
        heading_1: HeadingBlock,
    },
    #[serde(rename = "heading_2")]
    Heading2 {
        heading_2: HeadingBlock,
    },
    #[serde(rename = "heading_3")]
    Heading3 {
        heading_3: HeadingBlock,
    },
    BulletedListItem {
        bulleted_list_item: TextBlock,
    },
    NumberedListItem {
        numbered_list_item: TextBlock,
    },
    ToDo {
        to_do: ToDoBlock,
    },
    Toggle {
        toggle: TextBlock,
    },
    Code {
        code: CodeBlock,
    },
    Quote {
        quote: TextBlock,
    },
    Callout {
        callout: CalloutBlock,
    },
    Divider {
        divider: EmptyBlock,
    },
}

// === Content shapes =======================================================

/// Content common to paragraph, list items, toggle, quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TextBlock {
    #[serde(default)]
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeadingBlock {
    #[serde(default)]
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub color: Color,
    #[serde(default)]
    pub is_toggleable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToDoBlock {
    #[serde(default)]
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub color: Color,
    #[serde(default)]
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CodeBlock {
    #[serde(default)]
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub caption: Vec<RichText>,
    /// Language identifier. Notion accepts a fixed enum on the wire
    /// but we keep it as String to let agents pass new values through.
    #[serde(default = "default_code_language")]
    pub language: String,
}

fn default_code_language() -> String {
    "plain text".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CalloutBlock {
    #[serde(default)]
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub color: Color,
    /// Optional icon — shape varies (emoji, external, file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<serde_json::Value>,
}

/// Divider has no content; the `divider` field is an empty object on
/// the wire. We model it as a unit struct but serialise as `{}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmptyBlock {}

// === Convenience constructors ============================================

impl BlockBody {
    /// `paragraph` with a single plain-text run.
    #[must_use]
    pub fn paragraph(text: &str) -> Self {
        Self::Paragraph {
            paragraph: TextBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
            },
        }
    }

    #[must_use]
    pub fn heading_1(text: &str) -> Self {
        Self::Heading1 {
            heading_1: HeadingBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
                is_toggleable: false,
            },
        }
    }

    #[must_use]
    pub fn heading_2(text: &str) -> Self {
        Self::Heading2 {
            heading_2: HeadingBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
                is_toggleable: false,
            },
        }
    }

    #[must_use]
    pub fn heading_3(text: &str) -> Self {
        Self::Heading3 {
            heading_3: HeadingBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
                is_toggleable: false,
            },
        }
    }

    #[must_use]
    pub fn bulleted(text: &str) -> Self {
        Self::BulletedListItem {
            bulleted_list_item: TextBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
            },
        }
    }

    #[must_use]
    pub fn numbered(text: &str) -> Self {
        Self::NumberedListItem {
            numbered_list_item: TextBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
            },
        }
    }

    #[must_use]
    pub fn to_do(text: &str, checked: bool) -> Self {
        Self::ToDo {
            to_do: ToDoBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
                checked,
            },
        }
    }

    #[must_use]
    pub fn code(text: &str, language: &str) -> Self {
        Self::Code {
            code: CodeBlock {
                rich_text: vec![plain_rich_text(text)],
                caption: vec![],
                language: language.to_string(),
            },
        }
    }

    #[must_use]
    pub fn quote(text: &str) -> Self {
        Self::Quote {
            quote: TextBlock {
                rich_text: vec![plain_rich_text(text)],
                color: Color::Default,
            },
        }
    }

    #[must_use]
    pub fn divider() -> Self {
        Self::Divider { divider: EmptyBlock {} }
    }
}

fn plain_rich_text(text: &str) -> RichText {
    use crate::types::rich_text::{Annotations, RichTextContent, TextContent};
    RichText {
        content: RichTextContent::Text {
            text: TextContent {
                content: text.to_string(),
                link: None,
            },
        },
        annotations: Annotations::default(),
        plain_text: text.to_string(),
        href: None,
    }
}

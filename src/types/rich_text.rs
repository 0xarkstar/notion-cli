//! Notion rich text — the building block of titles, text blocks, and
//! many property values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::common::Color;

/// A single rich-text run.
///
/// Wire format:
/// ```json
/// {
///   "type": "text",
///   "text": {"content": "Hello", "link": null},
///   "annotations": {"bold": false, ...},
///   "plain_text": "Hello",
///   "href": null
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RichText {
    #[serde(flatten)]
    pub content: RichTextContent,
    #[serde(default)]
    pub annotations: Annotations,
    pub plain_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

impl RichText {
    /// Build a single-run plain-text vector — the canonical write-path
    /// form for titles and one-liner rich-text fields.
    #[must_use]
    pub fn plain(s: &str) -> Vec<Self> {
        vec![Self {
            content: RichTextContent::Text {
                text: TextContent { content: s.to_string(), link: None },
            },
            annotations: Annotations::default(),
            plain_text: s.to_string(),
            href: None,
        }]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RichTextContent {
    Text { text: TextContent },
    Mention { mention: serde_json::Value },
    Equation { equation: EquationContent },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TextContent {
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Link {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EquationContent {
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Annotations {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub strikethrough: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub code: bool,
    #[serde(default)]
    pub color: Color,
}

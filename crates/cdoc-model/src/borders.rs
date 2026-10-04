//! Borders and shading, shared by paragraphs, tables and cells.

use crate::units::{BorderWidth, Color, Length};
use serde::{Deserialize, Serialize};

/// The line style of one border edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderStyle {
    /// No line; the edge is explicitly borderless (overrides an inherited border).
    None,
    /// A single line.
    Single,
    /// A thick single line.
    Thick,
    /// Two lines.
    Double,
    /// Dotted.
    Dotted,
    /// Dashed.
    Dashed,
    /// Alternating dots and dashes.
    DotDash,
    /// Three lines.
    Triple,
}

/// One border edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Border {
    /// The line style.
    pub style: BorderStyle,
    /// The line width.
    pub width: BorderWidth,
    /// The line colour; `None` is "automatic" (the text colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Space between the border and the content.
    pub space: Length,
}

/// The four edges of a paragraph, table or cell, plus the edge between consecutive bordered
/// paragraphs (Word's `between`). Each edge is independently optional: `None` inherits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Borders {
    /// Top edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<Border>,
    /// Bottom edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Border>,
    /// Left edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Border>,
    /// Right edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Border>,
    /// The edge drawn between two consecutive paragraphs sharing the same borders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Border>,
}

impl Borders {
    /// No edge set; every edge inherits.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            top: None,
            bottom: None,
            left: None,
            right: None,
            between: None,
        }
    }

    /// The same border on all four outer edges.
    #[must_use]
    pub fn all(border: Border) -> Self {
        Self {
            top: Some(border.clone()),
            bottom: Some(border.clone()),
            left: Some(border.clone()),
            right: Some(border),
            between: None,
        }
    }
}

/// Background fill behind a paragraph or cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shading {
    /// The fill colour.
    pub fill: Color,
}

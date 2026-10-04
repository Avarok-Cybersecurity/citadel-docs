//! Paragraphs and paragraph properties.

use crate::borders::{Borders, Shading};
use crate::ids::{NumId, StyleId};
use crate::inline::Inline;
use crate::units::Length;
use serde::{Deserialize, Serialize};

/// Horizontal alignment of lines in a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Alignment {
    /// Left-aligned (start).
    Left,
    /// Centred.
    Center,
    /// Right-aligned (end).
    Right,
    /// Justified.
    Justify,
}

/// Line spacing, as Word's paragraph dialog offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "camelCase")]
pub enum LineSpacing {
    /// A multiple of single spacing, in **240ths**: 240 is single, 276 is 1.15, 360 is 1.5,
    /// 480 is double. Word's own unit, so a `.docx` value survives unchanged.
    Multiple {
        /// 240ths of a line.
        lines: u32,
    },
    /// Exactly this tall.
    Exact {
        /// The line height.
        height: Length,
    },
    /// At least this tall.
    AtLeast {
        /// The minimum line height.
        height: Length,
    },
}

/// Space around and within a paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spacing {
    /// Space before the paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Length>,
    /// Space after the paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Length>,
    /// Line spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSpacing>,
}

/// How the first line differs from the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FirstLine {
    /// The first line starts further right than the others.
    Indent {
        /// The extra indent.
        by: Length,
    },
    /// The first line starts further left than the others (lists, bibliographies).
    Hanging {
        /// The hang.
        by: Length,
    },
}

/// Indentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Indent {
    /// Left (start) indent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Length>,
    /// Right (end) indent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Length>,
    /// First-line indent or hang.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_line: Option<FirstLine>,
}

/// Membership of a paragraph in a multilevel list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingRef {
    /// The numbering instance.
    pub num_id: NumId,
    /// The level, `0..=8`.
    pub level: u8,
}

/// Paragraph formatting. Direct formatting on a paragraph, or the paragraph half of a style.
/// Every field is optional: `None` inherits from the style cascade.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphProperties {
    /// The paragraph style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleId>,
    /// Alignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Alignment>,
    /// Spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<Spacing>,
    /// Indentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<Indent>,
    /// Borders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borders: Option<Borders>,
    /// Shading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<Shading>,
    /// Keep on the same page as the next paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_with_next: Option<bool>,
    /// Keep all lines on one page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    /// Start a new page before this paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    /// List membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<NumberingRef>,
    /// Outline level, `0..=8`, for the navigation pane and tables of contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<u8>,
}

impl ParagraphProperties {
    /// No direct formatting at all; everything inherits.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            style: None,
            alignment: None,
            spacing: None,
            indent: None,
            borders: None,
            shading: None,
            keep_with_next: None,
            keep_lines: None,
            page_break_before: None,
            numbering: None,
            outline_level: None,
        }
    }

    /// Whether no field is set.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::none()
    }

    /// `over` applied on top of `self`; one step of the style cascade. Compound fields
    /// (spacing, indent, borders) are replaced whole when set, as Word does.
    #[must_use]
    pub fn overlay(&self, over: &Self) -> Self {
        Self {
            style: over.style.clone().or_else(|| self.style.clone()),
            alignment: over.alignment.or(self.alignment),
            spacing: over.spacing.clone().or_else(|| self.spacing.clone()),
            indent: over.indent.clone().or_else(|| self.indent.clone()),
            borders: over.borders.clone().or_else(|| self.borders.clone()),
            shading: over.shading.clone().or_else(|| self.shading.clone()),
            keep_with_next: over.keep_with_next.or(self.keep_with_next),
            keep_lines: over.keep_lines.or(self.keep_lines),
            page_break_before: over.page_break_before.or(self.page_break_before),
            numbering: over.numbering.or(self.numbering),
            outline_level: over.outline_level.or(self.outline_level),
        }
    }
}

/// A paragraph: properties plus inline content. An empty paragraph has no inlines.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph {
    /// Direct paragraph formatting.
    pub properties: ParagraphProperties,
    /// The content, in reading order.
    pub inlines: Vec<Inline>,
}

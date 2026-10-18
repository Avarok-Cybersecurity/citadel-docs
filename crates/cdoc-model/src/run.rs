//! Run (character) properties. Every field is optional: `None` inherits from the style
//! cascade, `Some(false)` explicitly turns an inherited property off.

use crate::ids::StyleId;
use crate::typography::Ligatures;
use crate::units::{Color, FontSize};
use serde::{Deserialize, Serialize};

/// The underline line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnderlineStyle {
    /// Explicitly no underline (overrides an inherited one).
    None,
    /// A single line.
    Single,
    /// A double line.
    Double,
    /// Thick.
    Thick,
    /// Dotted.
    Dotted,
    /// Dashed.
    Dashed,
    /// Wavy.
    Wave,
    /// Only under words, not spaces.
    Words,
}

/// An underline: style plus an optional colour (`None` is the text colour).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Underline {
    /// The line style.
    pub style: UnderlineStyle,
    /// The line colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

/// Vertical position of the run relative to the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerticalAlign {
    /// On the baseline (explicitly, overriding an inherited super/subscript).
    Baseline,
    /// Raised and reduced.
    Superscript,
    /// Lowered and reduced.
    Subscript,
}

/// Character formatting. Direct formatting on a run, or the run half of a style.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProperties {
    /// A character style applied to the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleId>,
    /// Font family name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Font size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<FontSize>,
    /// Bold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// Italic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Underline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<Underline>,
    /// Strikethrough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    /// Superscript, subscript or baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<VerticalAlign>,
    /// Text colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Highlight (background) colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Color>,
    /// Small capitals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    /// All capitals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_caps: Option<bool>,
    /// Kerning (`w:kern`): applied at this size and above; `FontSize(0)` switches it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kern: Option<FontSize>,
    /// The OpenType ligatures used (`w14:ligatures`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<Ligatures>,
    /// The language of the text, a BCP 47 tag (`w:lang`): hyphenation patterns follow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl RunProperties {
    /// No direct formatting at all; everything inherits.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            style: None,
            font: None,
            size: None,
            bold: None,
            italic: None,
            underline: None,
            strike: None,
            vertical_align: None,
            color: None,
            highlight: None,
            small_caps: None,
            all_caps: None,
            kern: None,
            ligatures: None,
            lang: None,
        }
    }

    /// Whether no field is set.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::none()
    }

    /// `over` applied on top of `self`: each field of `over` that is set replaces the field in
    /// `self`. This is one step of the style cascade.
    #[must_use]
    pub fn overlay(&self, over: &Self) -> Self {
        Self {
            style: over.style.clone().or_else(|| self.style.clone()),
            font: over.font.clone().or_else(|| self.font.clone()),
            size: over.size.or(self.size),
            bold: over.bold.or(self.bold),
            italic: over.italic.or(self.italic),
            underline: over.underline.clone().or_else(|| self.underline.clone()),
            strike: over.strike.or(self.strike),
            vertical_align: over.vertical_align.or(self.vertical_align),
            color: over.color.or(self.color),
            highlight: over.highlight.or(self.highlight),
            small_caps: over.small_caps.or(self.small_caps),
            all_caps: over.all_caps.or(self.all_caps),
            kern: over.kern.or(self.kern),
            ligatures: over.ligatures.or(self.ligatures),
            lang: over.lang.clone().or_else(|| self.lang.clone()),
        }
    }
}

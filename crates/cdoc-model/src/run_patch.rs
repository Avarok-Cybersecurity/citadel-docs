//! A change to a run's direct character properties, field by field: what the font group of
//! the ribbon sends. `null` clears a field (no highlight, the style's colour), so Clear
//! Formatting is [`RunPatch::clear_all`]. A `RunProperties` never holds `null`, so one read
//! as a patch sets exactly the fields it has: a version-1 payload means what it meant.

use crate::ids::StyleId;
use crate::patch::{apply, patch, set};
use crate::run::{RunProperties, Underline, VerticalAlign};
use crate::units::{Color, FontSize};

patch!(
    /// A change to a run's direct properties.
    RunPatch {
        /// Character style.
        style: StyleId,
        /// Font family.
        font: String,
        /// Font size.
        size: FontSize,
        /// Bold.
        bold: bool,
        /// Italic.
        italic: bool,
        /// Underline.
        underline: Underline,
        /// Strikethrough.
        strike: bool,
        /// Superscript or subscript.
        vertical_align: VerticalAlign,
        /// Text colour.
        color: Color,
        /// Highlight.
        highlight: Color,
        /// Small capitals.
        small_caps: bool,
        /// All capitals.
        all_caps: bool,
    }
);

impl RunPatch {
    /// Whether the patch changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Clears every direct property: Clear Formatting.
    #[must_use]
    pub const fn clear_all() -> Self {
        Self {
            style: Some(None),
            font: Some(None),
            size: Some(None),
            bold: Some(None),
            italic: Some(None),
            underline: Some(None),
            strike: Some(None),
            vertical_align: Some(None),
            color: Some(None),
            highlight: Some(None),
            small_caps: Some(None),
            all_caps: Some(None),
        }
    }

    /// `p` with the patch applied.
    #[must_use]
    pub fn apply(&self, p: &RunProperties) -> RunProperties {
        RunProperties {
            style: apply(self.style.as_ref(), p.style.as_ref()),
            font: apply(self.font.as_ref(), p.font.as_ref()),
            size: apply(self.size.as_ref(), p.size.as_ref()),
            bold: apply(self.bold.as_ref(), p.bold.as_ref()),
            italic: apply(self.italic.as_ref(), p.italic.as_ref()),
            underline: apply(self.underline.as_ref(), p.underline.as_ref()),
            strike: apply(self.strike.as_ref(), p.strike.as_ref()),
            vertical_align: apply(self.vertical_align.as_ref(), p.vertical_align.as_ref()),
            color: apply(self.color.as_ref(), p.color.as_ref()),
            highlight: apply(self.highlight.as_ref(), p.highlight.as_ref()),
            small_caps: apply(self.small_caps.as_ref(), p.small_caps.as_ref()),
            all_caps: apply(self.all_caps.as_ref(), p.all_caps.as_ref()),
        }
    }

    /// The values the patch sets (its clears are absent).
    #[must_use]
    pub fn sets(&self) -> RunProperties {
        self.apply(&RunProperties::none())
    }

    /// Which fields the patch clears, in field order.
    #[must_use]
    pub const fn clears(&self) -> [bool; 12] {
        [
            matches!(self.style, Some(None)),
            matches!(self.font, Some(None)),
            matches!(self.size, Some(None)),
            matches!(self.bold, Some(None)),
            matches!(self.italic, Some(None)),
            matches!(self.underline, Some(None)),
            matches!(self.strike, Some(None)),
            matches!(self.vertical_align, Some(None)),
            matches!(self.color, Some(None)),
            matches!(self.highlight, Some(None)),
            matches!(self.small_caps, Some(None)),
            matches!(self.all_caps, Some(None)),
        ]
    }
}

impl From<&RunProperties> for RunPatch {
    /// The patch that sets every field `p` has.
    fn from(p: &RunProperties) -> Self {
        Self {
            style: set(p.style.as_ref()),
            font: set(p.font.as_ref()),
            size: set(p.size.as_ref()),
            bold: set(p.bold.as_ref()),
            italic: set(p.italic.as_ref()),
            underline: set(p.underline.as_ref()),
            strike: set(p.strike.as_ref()),
            vertical_align: set(p.vertical_align.as_ref()),
            color: set(p.color.as_ref()),
            highlight: set(p.highlight.as_ref()),
            small_caps: set(p.small_caps.as_ref()),
            all_caps: set(p.all_caps.as_ref()),
        }
    }
}

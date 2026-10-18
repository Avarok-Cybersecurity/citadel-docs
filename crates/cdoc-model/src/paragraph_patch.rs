//! A change to a paragraph's direct properties, field by field: what the ribbons send.
//!
//! Every field has three states, which the JSON data model carries as absent, a value and
//! `null`: keep the paragraph's value, set it, clear it (back to the style's). `spacing` and
//! `indent` are patched inside: a patch naming only `spacing.line` keeps `before` and `after`.
//! A `ParagraphProperties` never holds `null`, so one read as a patch sets exactly the fields
//! it has.

use crate::borders::{Borders, Shading};
use crate::ids::StyleId;
use crate::paragraph::{
    Alignment, FirstLine, Indent, LineSpacing, NumberingRef, ParagraphProperties, Spacing,
};
use crate::patch::{apply, patch, set};
use crate::units::Length;

patch!(
    /// A change to `spacing`'s fields.
    SpacingPatch {
        /// Space before.
        before: Length,
        /// Space after.
        after: Length,
        /// Line spacing.
        line: LineSpacing,
    }
);

patch!(
    /// A change to `indent`'s fields.
    IndentPatch {
        /// Left indent.
        left: Length,
        /// Right indent.
        right: Length,
        /// First-line indent or hanging indent.
        first_line: FirstLine,
    }
);

patch!(
    /// A change to a paragraph's direct properties.
    ParagraphPatch {
        /// Paragraph style.
        style: StyleId,
        /// Alignment.
        alignment: Alignment,
        /// Spacing, patched field by field; `null` clears all of it.
        spacing: SpacingPatch,
        /// Indentation, patched field by field; `null` clears all of it.
        indent: IndentPatch,
        /// Borders, replaced whole.
        borders: Borders,
        /// Shading, replaced whole.
        shading: Shading,
        /// Keep with the next paragraph.
        keep_with_next: bool,
        /// Keep the lines together.
        keep_lines: bool,
        /// Start on a new page.
        page_break_before: bool,
        /// List membership.
        numbering: NumberingRef,
        /// Outline level.
        outline_level: u8,
    }
);

impl SpacingPatch {
    fn apply(&self, s: &Spacing) -> Option<Spacing> {
        let out = Spacing {
            before: apply(self.before.as_ref(), s.before.as_ref()),
            after: apply(self.after.as_ref(), s.after.as_ref()),
            line: apply(self.line.as_ref(), s.line.as_ref()),
        };
        (out.before.is_some() || out.after.is_some() || out.line.is_some()).then_some(out)
    }
}

impl IndentPatch {
    fn apply(&self, i: &Indent) -> Option<Indent> {
        let out = Indent {
            left: apply(self.left.as_ref(), i.left.as_ref()),
            right: apply(self.right.as_ref(), i.right.as_ref()),
            first_line: apply(self.first_line.as_ref(), i.first_line.as_ref()),
        };
        (out.left.is_some() || out.right.is_some() || out.first_line.is_some()).then_some(out)
    }
}

impl ParagraphPatch {
    /// Whether the patch changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `p` with the patch applied.
    #[must_use]
    pub fn apply(&self, p: &ParagraphProperties) -> ParagraphProperties {
        let empty_spacing = Spacing {
            before: None,
            after: None,
            line: None,
        };
        let empty_indent = Indent {
            left: None,
            right: None,
            first_line: None,
        };
        ParagraphProperties {
            style: apply(self.style.as_ref(), p.style.as_ref()),
            alignment: apply(self.alignment.as_ref(), p.alignment.as_ref()),
            spacing: match &self.spacing {
                None => p.spacing.clone(),
                Some(None) => None,
                Some(Some(s)) => s.apply(p.spacing.as_ref().unwrap_or(&empty_spacing)),
            },
            indent: match &self.indent {
                None => p.indent.clone(),
                Some(None) => None,
                Some(Some(i)) => i.apply(p.indent.as_ref().unwrap_or(&empty_indent)),
            },
            borders: apply(self.borders.as_ref(), p.borders.as_ref()),
            shading: apply(self.shading.as_ref(), p.shading.as_ref()),
            keep_with_next: apply(self.keep_with_next.as_ref(), p.keep_with_next.as_ref()),
            keep_lines: apply(self.keep_lines.as_ref(), p.keep_lines.as_ref()),
            page_break_before: apply(
                self.page_break_before.as_ref(),
                p.page_break_before.as_ref(),
            ),
            numbering: apply(self.numbering.as_ref(), p.numbering.as_ref()),
            outline_level: apply(self.outline_level.as_ref(), p.outline_level.as_ref()),
        }
    }
}

impl From<&ParagraphProperties> for ParagraphPatch {
    /// The patch that sets every field `p` has, inside `spacing` and `indent` too.
    fn from(p: &ParagraphProperties) -> Self {
        Self {
            style: set(p.style.as_ref()),
            alignment: set(p.alignment.as_ref()),
            spacing: p.spacing.as_ref().map(|s| {
                Some(SpacingPatch {
                    before: set(s.before.as_ref()),
                    after: set(s.after.as_ref()),
                    line: set(s.line.as_ref()),
                })
            }),
            indent: p.indent.as_ref().map(|i| {
                Some(IndentPatch {
                    left: set(i.left.as_ref()),
                    right: set(i.right.as_ref()),
                    first_line: set(i.first_line.as_ref()),
                })
            }),
            borders: set(p.borders.as_ref()),
            shading: set(p.shading.as_ref()),
            keep_with_next: set(p.keep_with_next.as_ref()),
            keep_lines: set(p.keep_lines.as_ref()),
            page_break_before: set(p.page_break_before.as_ref()),
            numbering: set(p.numbering.as_ref()),
            outline_level: set(p.outline_level.as_ref()),
        }
    }
}

//! Formatting at the caret, for the ribbons' pressed and indeterminate states: the effective
//! (style-resolved) and direct character and paragraph properties where the selection starts,
//! and which properties differ across the selection.

use crate::commands::{inherited_by, selected_paragraphs, texts};
use crate::error::EditError;
use crate::outline::Outline;
use crate::position::Selection;
use crate::run_keys::fields;
use crate::text::paragraph;
use cdoc_crdt::{atom_at, Atom, Doc, RunKey};
use cdoc_model::{ParagraphProperties, RunProperties, StyleSheet};

/// A paragraph property, in the order of `ParagraphProperties`' fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParagraphKey {
    /// `style`.
    Style,
    /// `alignment`.
    Alignment,
    /// `spacing`.
    Spacing,
    /// `indent`.
    Indent,
    /// `borders`.
    Borders,
    /// `shading`.
    Shading,
    /// `keep_with_next`.
    KeepWithNext,
    /// `keep_lines`.
    KeepLines,
    /// `page_break_before`.
    PageBreakBefore,
    /// `numbering`.
    Numbering,
    /// `outline_level`.
    OutlineLevel,
}

impl ParagraphKey {
    /// Every key, in field order (the bit order of the wire's mixed set).
    pub const ALL: [Self; 11] = [
        Self::Style,
        Self::Alignment,
        Self::Spacing,
        Self::Indent,
        Self::Borders,
        Self::Shading,
        Self::KeepWithNext,
        Self::KeepLines,
        Self::PageBreakBefore,
        Self::Numbering,
        Self::OutlineLevel,
    ];

    fn same(self, a: &ParagraphProperties, b: &ParagraphProperties) -> bool {
        match self {
            Self::Style => a.style == b.style,
            Self::Alignment => a.alignment == b.alignment,
            Self::Spacing => a.spacing == b.spacing,
            Self::Indent => a.indent == b.indent,
            Self::Borders => a.borders == b.borders,
            Self::Shading => a.shading == b.shading,
            Self::KeepWithNext => a.keep_with_next == b.keep_with_next,
            Self::KeepLines => a.keep_lines == b.keep_lines,
            Self::PageBreakBefore => a.page_break_before == b.page_break_before,
            Self::Numbering => a.numbering == b.numbering,
            Self::OutlineLevel => a.outline_level == b.outline_level,
        }
    }
}

/// Formatting where the selection starts. The paragraph style id and the list (`num_id`,
/// `level`) are `paragraph.style` and `paragraph.numbering`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaretFormatting {
    /// Character properties after the cascade (defaults, paragraph style, character style,
    /// direct): what the text shows. For a collapsed selection, what typed text will get.
    pub run: RunProperties,
    /// The direct character properties (pending formatting included).
    pub run_direct: RunProperties,
    /// Paragraph properties after the cascade.
    pub paragraph: ParagraphProperties,
    /// The paragraph's direct properties.
    pub paragraph_direct: ParagraphProperties,
    /// Character properties whose effective value differs somewhere in the selection.
    pub mixed_run: Vec<RunKey>,
    /// Paragraph properties whose effective value differs between the selected paragraphs.
    pub mixed_paragraph: Vec<ParagraphKey>,
}

fn paragraph_at(doc: &Doc, index: usize) -> Result<ParagraphProperties, EditError> {
    match atom_at(doc, index)? {
        Some(Atom::Paragraph(p)) => Ok(p),
        _ => Err(EditError::NotApplicable(
            "the position is not in a paragraph",
        )),
    }
}

/// The formatting of `selection`; `pending` is the formatting chosen for the next typed text,
/// `props_at` reads the direct character properties of a unit (`None` if it is not text).
pub(crate) fn caret_formatting(
    doc: &Doc,
    outline: &Outline,
    styles: &StyleSheet,
    selection: Selection,
    pending: Option<&RunProperties>,
    props_at: impl Fn(usize) -> Result<Option<RunProperties>, EditError>,
) -> Result<CaretFormatting, EditError> {
    let range = selection.range(doc)?;
    let (_, para) = paragraph(doc, outline, range.start)?;
    let paragraph_direct = paragraph_at(doc, para.start - 1)?;
    let runs = if range.is_empty() {
        Vec::new()
    } else {
        texts(doc, outline, range.clone())?
    };
    let (run_para, run_direct) = match runs.first() {
        Some((_, p, r)) => (p.clone(), r.clone()),
        None => (
            paragraph_direct.clone(),
            inherited_by(pending, range.start, para.start, props_at)?,
        ),
    };
    let run = styles.resolve_run(&run_para, &run_direct)?;
    let paragraph = styles.resolve_paragraph(&paragraph_direct)?;
    let mut mixed_run = Vec::new();
    for (_, p, r) in runs.iter().skip(1) {
        let other = fields(&styles.resolve_run(p, r)?);
        for ((key, a), (_, b)) in fields(&run).iter().zip(&other) {
            if a != b && !mixed_run.contains(key) {
                mixed_run.push(*key);
            }
        }
    }
    mixed_run.sort_by_key(|k| RunKey::ALL.iter().position(|a| a == k));
    let mut mixed_paragraph = Vec::new();
    if !range.is_empty() {
        for at in selected_paragraphs(doc, outline, &range)? {
            let other = styles.resolve_paragraph(&paragraph_at(doc, at)?)?;
            for key in ParagraphKey::ALL {
                if !key.same(&paragraph, &other) && !mixed_paragraph.contains(&key) {
                    mixed_paragraph.push(key);
                }
            }
        }
    }
    mixed_paragraph.sort_by_key(|k| ParagraphKey::ALL.iter().position(|a| a == k));
    Ok(CaretFormatting {
        run,
        run_direct,
        paragraph,
        paragraph_direct,
        mixed_run,
        mixed_paragraph,
    })
}

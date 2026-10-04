//! What every stage of the layout shares: the document, the faces, the options, and the
//! numbering of footnotes as they are met.

use crate::fonts::{sanitise, Fonts};
use crate::ir::FaceId;
use cdoc_model::{Document, FootnoteId, Length};
use std::collections::BTreeMap;

/// How tracked changes are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionView {
    /// Insertions underlined and deletions struck through, both in the markup colour.
    Markup,
    /// As if every change were accepted: deletions are not set at all.
    Final,
}

/// Choices the model does not make, made explicitly by the caller. No `Default`: a preset is
/// named at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutOptions {
    /// The size of a signature field's box.
    pub signature_box: (Length, Length),
    /// Cell padding (left and right) for a table that does not set one.
    pub cell_padding: Length,
    /// How tracked changes are shown.
    pub revisions: RevisionView,
}

impl LayoutOptions {
    /// Word's look: 0.08 in cell padding, changes shown as markup, a 3 in × 0.5 in signature
    /// box.
    #[must_use]
    pub const fn word_like() -> Self {
        Self {
            signature_box: (Length(21_600), Length(3_600)),
            cell_padding: Length::from_twips(108),
            revisions: RevisionView::Markup,
        }
    }
}

/// Values for page-dependent fields, known while a header or footer is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldValues {
    /// The page number as printed.
    pub page: String,
    /// The number of pages.
    pub pages: String,
}

/// Shared state.
pub struct Ctx<'d, 'f> {
    /// The document.
    pub doc: &'d Document,
    /// Faces in use.
    pub fonts: Fonts<'f>,
    /// Options.
    pub options: &'d LayoutOptions,
    /// Footnote numbers, assigned in order of first reference.
    pub notes: BTreeMap<FootnoteId, u32>,
    /// Characters replaced by the marker, per face.
    pub missing: BTreeMap<FaceId, usize>,
    /// Set while a header or footer is laid out.
    pub fields: Option<FieldValues>,
}

impl<'d, 'f> Ctx<'d, 'f> {
    /// Fresh state.
    #[must_use]
    pub const fn new(doc: &'d Document, fonts: Fonts<'f>, options: &'d LayoutOptions) -> Self {
        Self {
            doc,
            fonts,
            options,
            notes: BTreeMap::new(),
            missing: BTreeMap::new(),
            fields: None,
        }
    }

    /// The footnote's number, assigned on first reference.
    pub fn note_number(&mut self, id: FootnoteId) -> u32 {
        let next = u32::try_from(self.notes.len()).unwrap_or(u32::MAX - 1) + 1;
        *self.notes.entry(id).or_insert(next)
    }

    /// Text the face can draw; replaced characters are counted for the warnings.
    pub fn drawable(&mut self, text: &str, face: FaceId) -> String {
        let (out, replaced) = sanitise(text, &self.fonts, face);
        if replaced > 0 {
            *self.missing.entry(face).or_insert(0) += replaced;
        }
        out
    }
}

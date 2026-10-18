//! What every stage of the layout shares: the document, the faces, the options, and the
//! numbering of footnotes as they are met.

use crate::fonts::{sanitise, Fonts};
use crate::hyphens::Hyphenators;
use crate::ir::{FaceId, PlacedGlyph, Pt100};
use crate::style::TextStyle;
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

/// How paragraphs are broken into lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BreakerChoice {
    /// Line by line, as a word processor sets them: each line takes every word that fits.
    /// Spaces stretch on justified lines and never shrink, as Word's do.
    FirstFit,
    /// Knuth and Plass's optimum fit over the whole paragraph with these parameters: spaces
    /// stretch by half and shrink by a third of their width (TeX's interword glue), so
    /// justified lines are spaced evenly and loose lines are avoided.
    KnuthPlass(cdoc_para::Params),
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
    /// How paragraphs are broken into lines.
    pub breaker: BreakerChoice,
}

impl LayoutOptions {
    /// Word's look (0.08 in cell padding, changes shown as markup, a 3 in × 0.5 in signature
    /// box) with lines broken by `breaker`.
    #[must_use]
    pub const fn word_like(breaker: BreakerChoice) -> Self {
        Self {
            signature_box: (Length(21_600), Length(3_600)),
            cell_padding: Length::from_twips(108),
            revisions: RevisionView::Markup,
            breaker,
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

/// What one stretch of work (a block measured, a block placed, a page furnished) did to the
/// shared state: the face requests it made, the characters it replaced, the footnotes it
/// numbered. An incremental layout keeps these per stretch so it can tell what a fresh layout
/// would have met, and in which order, without redoing the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// Face request indices ([`Fonts::take_requests`]), each once, first made first.
    pub requests: Vec<usize>,
    /// Characters replaced by the marker, per face.
    pub missing: Vec<(FaceId, usize)>,
    /// Footnotes numbered, in numbering order.
    pub notes: Vec<FootnoteId>,
    /// Languages met in hyphenated text that have no patterns, each once, first met first.
    pub unhyphenated: Vec<String>,
}

impl Trace {
    /// A stretch that did nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            requests: Vec::new(),
            missing: Vec::new(),
            notes: Vec::new(),
            unhyphenated: Vec::new(),
        }
    }
}

/// Shared state.
pub struct Ctx<'d, 'f> {
    /// The document.
    pub doc: &'d Document,
    /// Faces in use.
    pub fonts: Fonts<'f>,
    /// Hyphenation patterns, compiled once each.
    pub hyphenators: Hyphenators,
    /// Options.
    pub options: &'d LayoutOptions,
    /// Footnote numbers, assigned in order of first reference.
    pub notes: BTreeMap<FootnoteId, u32>,
    /// The footnotes in numbering order.
    pub order: Vec<FootnoteId>,
    /// Footnotes numbered since the last [`Ctx::take_trace`].
    numbered: Vec<FootnoteId>,
    /// Characters replaced by the marker, per face, since the last [`Ctx::take_trace`].
    missing: BTreeMap<FaceId, usize>,
    /// Languages without patterns met since the last [`Ctx::take_trace`].
    unhyphenated: Vec<String>,
    /// Set while a header or footer is laid out.
    pub fields: Option<FieldValues>,
}

impl<'d, 'f> Ctx<'d, 'f> {
    /// Fresh state.
    #[must_use]
    pub const fn new(
        doc: &'d Document,
        fonts: Fonts<'f>,
        hyphenators: Hyphenators,
        options: &'d LayoutOptions,
    ) -> Self {
        Self {
            doc,
            fonts,
            hyphenators,
            unhyphenated: Vec::new(),
            options,
            notes: BTreeMap::new(),
            order: Vec::new(),
            numbered: Vec::new(),
            missing: BTreeMap::new(),
            fields: None,
        }
    }

    /// The footnote's number, assigned on first reference.
    pub fn note_number(&mut self, id: FootnoteId) -> u32 {
        if let Some(number) = self.notes.get(&id) {
            return *number;
        }
        self.numbered.push(id);
        self.number_quietly(id)
    }

    /// Numbers a footnote without recording it in the current trace: replaying a trace that
    /// already holds it.
    pub fn number_quietly(&mut self, id: FootnoteId) -> u32 {
        let next = u32::try_from(self.notes.len()).unwrap_or(u32::MAX - 1) + 1;
        let number = *self.notes.entry(id).or_insert(next);
        if number == next {
            self.order.push(id);
        }
        number
    }

    /// Footnote numbering as it stood when `order` had been numbered.
    pub fn renumber(&mut self, order: &[FootnoteId]) {
        self.notes.clear();
        self.order.clear();
        for id in order {
            self.number_quietly(*id);
        }
    }

    /// What was done since the last call.
    pub fn take_trace(&mut self) -> Trace {
        Trace {
            requests: self.fonts.take_requests(),
            missing: std::mem::take(&mut self.missing).into_iter().collect(),
            notes: std::mem::take(&mut self.numbered),
            unhyphenated: std::mem::take(&mut self.unhyphenated),
        }
    }

    /// Records text in a language with no patterns, hyphenation being on.
    pub fn unhyphenated(&mut self, language: &str) {
        if !self.unhyphenated.iter().any(|l| l == language) {
            self.unhyphenated.push(language.to_owned());
        }
    }

    /// `text` as the style sets it: reduced to what the face can draw, shaped with the
    /// style's features at its size. Returns the text drawn, its glyphs and its width.
    pub fn set(&mut self, text: &str, style: &TextStyle) -> (String, Vec<PlacedGlyph>, Pt100) {
        let text = self.drawable(text, style.face);
        let mut glyphs = Vec::new();
        let width = self.fonts.shape(
            style.face,
            style.size,
            &text,
            style.features,
            0,
            &mut glyphs,
        );
        (text, glyphs, width)
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

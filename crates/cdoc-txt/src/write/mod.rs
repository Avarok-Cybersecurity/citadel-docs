//! Model to plain text.

mod block;
mod inline;
mod table;

use cdoc_layout::labels::Counters;
use cdoc_model::{Document, Report};

/// The part name losses are filed under.
pub(crate) const PART: &str = "text";

/// What every writer function needs.
pub(super) struct Ctx<'a> {
    doc: &'a Document,
    report: &'a mut Report,
    counters: Counters<'a>,
}

impl Ctx<'_> {
    pub(super) fn lose(&mut self, what: &str) {
        self.report.note(PART, what);
    }
}

/// Write a document as plain text. Paragraphs are separated by a blank line; list items carry
/// their computed labels and are indented two spaces per level; tables are aligned columns
/// with a rule under the header row; footnotes follow the body as `[N] text`. The report lists,
/// by kind, everything plain text could not hold.
#[must_use]
pub fn document_to_txt(doc: &Document) -> (String, Report) {
    let mut report = Report::empty();
    let mut ctx = Ctx {
        doc,
        report: &mut report,
        counters: Counters::new(&doc.numbering),
    };
    let text = block::document(&mut ctx);
    (text, report)
}

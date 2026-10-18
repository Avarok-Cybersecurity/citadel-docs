//! Model to Markdown.

mod block;
mod escape;
mod formatting;
mod inline;
mod pieces;
mod table;

pub use pieces::{CODE_FONT, MONOSPACE};

use cdoc_layout::labels::Counters;
use cdoc_model::{Document, NumId, Report};

/// The part name losses are filed under.
pub(crate) const PART: &str = "markdown";

/// What every writer function needs.
pub(super) struct Ctx<'a> {
    doc: &'a Document,
    report: &'a mut Report,
    counters: Counters<'a>,
    /// The nesting level of the list item just written, while a list is running.
    list_level: Option<usize>,
    /// Per nesting level, the ordered list last written there and its delimiter: an ordered
    /// list right after another (a restart) switches between `.` and `)`, the only way
    /// CommonMark starts a new list instead of counting on.
    ordered: Vec<Option<(NumId, char)>>,
    /// Whether text is being written inside a table cell, where `|` must be escaped even in
    /// code spans.
    in_table: bool,
}

impl Ctx<'_> {
    pub(super) fn lose(&mut self, what: &str) {
        self.report.note(PART, what);
    }
}

/// Write a document as CommonMark with GFM tables, strikethrough and footnotes. The report
/// lists, by kind, everything Markdown could not hold.
#[must_use]
pub fn document_to_md(doc: &Document) -> (String, Report) {
    let mut report = Report::empty();
    let mut ctx = Ctx {
        doc,
        report: &mut report,
        counters: Counters::new(&doc.numbering),
        list_level: None,
        ordered: Vec::new(),
        in_table: false,
    };
    let text = block::document(&mut ctx);
    (text, report)
}

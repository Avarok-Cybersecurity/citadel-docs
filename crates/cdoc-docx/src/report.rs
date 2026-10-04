//! The logger the part readers and writers share: the model's [`Report`] plus the part
//! currently being processed.

use cdoc_model::Report;

/// A report plus the part currently being processed, so the property mappers can note a loss
/// without being told where they are.
#[derive(Debug)]
pub struct Log {
    /// The part being processed.
    pub part: String,
    /// The accumulating report.
    pub report: Report,
}

impl Log {
    /// A log starting in `part`.
    #[must_use]
    pub fn new(part: &str) -> Self {
        Self {
            part: part.to_owned(),
            report: Report::empty(),
        }
    }

    /// Note an element or sentence lost in the current part.
    pub fn lose(&mut self, what: &str) {
        self.report.note(&self.part, what);
    }

    /// Note a child element the mapper does not model.
    pub fn unknown(&mut self, element: &str) {
        self.lose(element);
    }
}

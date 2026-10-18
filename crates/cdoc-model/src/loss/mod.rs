//! How much each loss matters, and what to call it: the one table every importer, exporter and
//! the facade's JSON read the severity and plain-language label of a [`Loss`] from.
//!
//! - **Info**: nothing a reader of the document sees changes (metadata, application
//!   settings, editing marks, language tags, references to nothing tidied away).
//! - **Minor**: all the content is there, but some formatting, layout or structure is
//!   approximated or not kept.
//! - **Major**: content the reader would see (text, images, comments, notes, parts) is gone or
//!   changed in meaning.
//!
//! A loss is classified by its `what`: an exact sentence (`loss/sentences.rs`), a sentence with a
//! value in it (a pattern), an element or attribute path, or "part not read" by the part's
//! name (`loss/elements.rs`). A sentence nothing names is **Major**: an unexpected loss is never
//! hidden. The test `crates/cdoc-model/tests/loss_coverage.rs` checks every sentence the readers
//! and writers can record is named here.
//!
//! [`Loss`]: crate::report::Loss

mod elements;
mod sentences;

use serde::Serialize;

/// How much a loss matters to someone reading the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Nothing visible changes.
    Info,
    /// The content is all there; its formatting, layout or structure differs.
    Minor,
    /// Visible content is gone or changed.
    Major,
}

/// A loss's severity and the plain-language label a user is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    /// How much it matters.
    pub severity: Severity,
    /// What to call it, for someone who does not read XML.
    pub label: &'static str,
}

impl Kind {
    pub(crate) const fn new(severity: Severity, label: &'static str) -> Self {
        Self { severity, label }
    }
}

/// The plain-language labels, each defined once.
pub mod labels {
    /// Document properties: author, dates, keywords, statistics.
    pub const PROPERTIES: &str = "Document properties";
    /// Settings of the application that wrote the file.
    pub const SETTINGS: &str = "Application settings";
    /// Marks only an editor uses: revision ids, proofing marks, bookmarks.
    pub const EDITING_MARKS: &str = "Editing marks";
    /// Language tags and the formatting of other scripts than the text's.
    pub const SCRIPTS: &str = "Language and other-script settings";
    /// A value the file left out, filled in the way word processors do.
    pub const DEFAULT_ASSUMED: &str = "Missing setting filled in";
    /// A reference to something that does not exist, tidied away.
    pub const REPAIRED: &str = "Broken reference repaired";
    /// Macros (a VBA project) were removed; the document is unaffected.
    pub const MACROS: &str = "Macros removed";
    /// Formatting kept as the nearest thing the editor has.
    pub const FORMAT_APPROXIMATED: &str = "Formatting approximated";
    /// Formatting the editor or the format cannot hold.
    pub const FORMAT_NOT_KEPT: &str = "Formatting not kept";
    /// Page layout, columns or placement simplified.
    pub const LAYOUT_SIMPLIFIED: &str = "Layout simplified";
    /// Lists, tables, sections or notes restructured, their content kept.
    pub const STRUCTURE_SIMPLIFIED: &str = "Structure simplified";
    /// Tracked changes accepted or simplified.
    pub const TRACKED_CHANGES: &str = "Tracked changes simplified";
    /// A field or form field kept as its text.
    pub const FIELD_AS_TEXT: &str = "Field kept as text";
    /// An image drawn as a placeholder box.
    pub const IMAGE_PLACEHOLDER: &str = "Image shown as a placeholder";
    /// Text, a comment or a note that is not kept.
    pub const CONTENT_NOT_KEPT: &str = "Content not kept";
    /// An image or drawing that is not kept.
    pub const IMAGE_NOT_KEPT: &str = "Image or drawing not kept";
    /// A part of the package that was not read and may hold content.
    pub const PART_NOT_READ: &str = "Part of the file not read";
    /// A loss nothing in the table names: shown as major so it is never hidden.
    pub const UNCLASSIFIED: &str = "Content not carried";
}

/// The kind of a loss recorded in `part` as `what`.
#[must_use]
pub fn classify(part: &str, what: &str) -> Kind {
    if let Some(kind) = sentences::exact(what) {
        return kind;
    }
    if what == elements::PART_NOT_READ {
        return elements::part_not_read(part);
    }
    if let Some(kind) = sentences::pattern(what) {
        return kind;
    }
    if let Some(kind) = elements::path(what) {
        return kind;
    }
    Kind::new(Severity::Major, labels::UNCLASSIFIED)
}

/// The kind of removed macros.
pub const MACROS_REMOVED: Kind = Kind::new(Severity::Info, labels::MACROS);

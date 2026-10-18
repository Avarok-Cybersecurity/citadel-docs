//! What kind of file a package is, and what the caller asks a writer to produce. Shared by the
//! formats so a document and its template are told apart the same way everywhere.

use cdoc_model::{Document, Report};
use serde::Serialize;

/// A document or a template. A template is a document new files are created from; the content
/// is the same, only the package declares it differently. Macro-enabled kinds are never a
/// `Kind`: the readers refuse the macros and report the file as the kind that remains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// `.docx`, `.odt`.
    Document,
    /// `.dotx`, `.ott`.
    Template,
}

/// What a writer produces. No default: the caller states the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOptions {
    /// The kind of file.
    pub kind: Kind,
}

impl WriteOptions {
    /// A document (`.docx`, `.odt`).
    #[must_use]
    pub const fn document() -> Self {
        Self {
            kind: Kind::Document,
        }
    }

    /// A template (`.dotx`, `.ott`).
    #[must_use]
    pub const fn template() -> Self {
        Self {
            kind: Kind::Template,
        }
    }
}

/// What a reader returns: the document, what it could not carry, and what kind of file it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The document, normalised but not validated.
    pub document: Document,
    /// What the model could not hold.
    pub report: Report,
    /// Document or template.
    pub kind: Kind,
}

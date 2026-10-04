//! The OPC package: a ZIP of named parts, their relationships and their content types.

mod rels;
mod types;
mod zipio;

pub use rels::Rels;
pub use types::ContentTypes;
pub use zipio::{Package, ReadLimits};

use crate::error::DocxError;

/// Relationship type URIs.
pub mod rel_type {
    const BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
    /// The main document.
    pub const DOCUMENT: &str = "officeDocument";
    /// Styles.
    pub const STYLES: &str = "styles";
    /// Numbering.
    pub const NUMBERING: &str = "numbering";
    /// Settings.
    pub const SETTINGS: &str = "settings";
    /// Footnotes.
    pub const FOOTNOTES: &str = "footnotes";
    /// Comments.
    pub const COMMENTS: &str = "comments";
    /// Header.
    pub const HEADER: &str = "header";
    /// Footer.
    pub const FOOTER: &str = "footer";
    /// Comment threading and resolution (Word 2013).
    pub const COMMENTS_EXTENDED: &str =
        "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
    /// Core properties.
    pub const CORE: &str =
        "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
    /// The sidecar that carries the model's string ids.
    pub const IDS: &str = "https://citadel-docs.avarok.net/relationships/ids";

    /// The full URI for a short name, or the name itself when it already is one.
    #[must_use]
    pub fn uri(name: &str) -> String {
        if name.starts_with("http") {
            name.to_owned()
        } else {
            format!("{BASE}{name}")
        }
    }
}

/// The part a relationship target names, relative to the part holding the relationship.
#[must_use]
pub fn resolve(source_part: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut segments: Vec<&str> = source_part.split('/').collect();
    segments.pop();
    for piece in target.split('/') {
        match piece {
            "." | "" => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// The rels part for a part: `word/document.xml` has `word/_rels/document.xml.rels`.
#[must_use]
pub fn rels_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// A part's bytes, or [`DocxError::MissingPart`].
pub fn require<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], DocxError> {
    package
        .get(name)
        .ok_or_else(|| DocxError::MissingPart(name.to_owned()))
}

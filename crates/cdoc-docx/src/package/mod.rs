//! The OPC package: a ZIP of named parts, their relationships and their content types.

mod rels;
mod types;

pub use cdoc_pkg::{require, Compression, Lead, Package, ReadLimits};
pub use rels::Rels;
pub use types::ContentTypes;

/// OPC packages start with `[Content_Types].xml`.
pub const LEAD: Lead = Lead {
    name: "[Content_Types].xml",
    compression: Compression::Deflated,
};

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

/// Whether a part is a relationships part.
#[must_use]
pub fn is_rels(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rels"))
}

/// The rels part for a part: `word/document.xml` has `word/_rels/document.xml.rels`.
#[must_use]
pub fn rels_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

//! Documents, templates and their macro-enabled twins: one table maps the main part's content
//! type to what the package is, so `.docx`, `.dotx`, `.docm` and `.dotm` share every other
//! step of reading and writing. Macros are refused: their parts are never read, never carried
//! and never written, and each one is reported as [`cdoc_model::Loss::MacrosRemoved`].

use crate::error::DocxError;
use cdoc_pkg::Kind;

const DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";
const DOCUMENT_MACROS: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";
const TEMPLATE_MACROS: &str = "application/vnd.ms-word.template.macroEnabledTemplate.main+xml";

/// The main part's content type for each kind this crate writes (never a macro-enabled one).
const WRITTEN: [(Kind, &str); 2] = [(Kind::Document, DOCUMENT), (Kind::Template, TEMPLATE)];

/// Every main-part content type this crate reads: the kind and whether macros may be present.
const READ: [(&str, Kind, bool); 4] = [
    (DOCUMENT, Kind::Document, false),
    (TEMPLATE, Kind::Template, false),
    (DOCUMENT_MACROS, Kind::Document, true),
    (TEMPLATE_MACROS, Kind::Template, true),
];

/// Content types of the parts that hold a VBA project, its data or its signature.
const MACRO_TYPES: [&str; 4] = [
    "application/vnd.ms-office.vbaProject",
    "application/vnd.ms-word.vbaData+xml",
    "application/vnd.ms-office.vbaProjectSignature",
    "application/vnd.ms-office.vbaProjectSignatureAgile",
];

/// The main part's content type for a kind.
#[must_use]
pub fn main_content_type(kind: Kind) -> &'static str {
    WRITTEN
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(DOCUMENT, |(_, ct)| ct)
}

/// What the main part's declared content type says the package is. A package that declares
/// none is read as a document; one that declares something that is not `WordprocessingML` is
/// refused.
pub fn read_kind(part: &str, declared: Option<&str>) -> Result<Kind, DocxError> {
    let Some(declared) = declared else {
        return Ok(Kind::Document);
    };
    READ.iter()
        .find(|(ct, _, _)| declared.eq_ignore_ascii_case(ct))
        .map(|(_, kind, _)| *kind)
        .ok_or_else(|| DocxError::Invalid {
            part: part.to_owned(),
            detail: format!("not a WordprocessingML main part: {declared}"),
        })
}

/// Whether a part holds macros: by its declared type, or by the names Word gives them when a
/// package declares no type for them.
#[must_use]
pub fn is_macro_part(name: &str, declared: Option<&str>) -> bool {
    if crate::package::is_rels(name) {
        return false;
    }
    let file = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    declared.is_some_and(|ct| MACRO_TYPES.iter().any(|m| ct.eq_ignore_ascii_case(m)))
        || file.starts_with("vbaproject")
        || file == "vbadata.xml"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_written_kind_reads_back_as_itself() {
        for kind in [Kind::Document, Kind::Template] {
            let ct = main_content_type(kind);
            assert_eq!(read_kind("word/document.xml", Some(ct)), Ok(kind));
        }
    }

    #[test]
    fn macro_enabled_packages_read_as_the_kind_that_remains() {
        assert_eq!(read_kind("m", Some(DOCUMENT_MACROS)), Ok(Kind::Document));
        assert_eq!(read_kind("m", Some(TEMPLATE_MACROS)), Ok(Kind::Template));
        assert_eq!(read_kind("m", None), Ok(Kind::Document));
        assert!(read_kind("m", Some("application/vnd.ms-excel")).is_err());
    }

    #[test]
    fn macro_parts_are_found_by_type_or_by_name() {
        assert!(is_macro_part("word/x.bin", Some(MACRO_TYPES[0])));
        assert!(is_macro_part("word/vbaData.xml", Some("application/xml")));
        assert!(is_macro_part("word/vbaProject.bin", None));
        assert!(!is_macro_part("word/document.xml", Some(DOCUMENT)));
        assert!(!is_macro_part("word/_rels/vbaProject.bin.rels", None));
        assert!(!is_macro_part("word/media/image1.png", Some("image/png")));
    }
}

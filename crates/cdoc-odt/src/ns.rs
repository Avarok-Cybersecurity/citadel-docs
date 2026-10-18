//! Namespaces, media types and the root elements every part starts from.

use cdoc_pkg::xml::Node;
use cdoc_pkg::Kind;

/// The ODF version written.
pub const VERSION: &str = "1.2";

const TEXT_DOCUMENT: &str = "application/vnd.oasis.opendocument.text";
const TEXT_TEMPLATE: &str = "application/vnd.oasis.opendocument.text-template";

/// The media type of each kind.
const KINDS: [(Kind, &str); 2] = [
    (Kind::Document, TEXT_DOCUMENT),
    (Kind::Template, TEXT_TEMPLATE),
];

/// The package media type for a kind (the `mimetype` entry and the manifest's root entry).
#[must_use]
pub fn media_type(kind: Kind) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(TEXT_DOCUMENT, |(_, mt)| mt)
}

/// The kind a package media type names, if it is a text document or template.
#[must_use]
pub fn kind_of(media_type: &str) -> Option<Kind> {
    KINDS
        .iter()
        .find(|(_, mt)| *mt == media_type.trim())
        .map(|(k, _)| *k)
}

/// Every namespace the parts use, by prefix.
const NAMESPACES: [(&str, &str); 12] = [
    ("office", "urn:oasis:names:tc:opendocument:xmlns:office:1.0"),
    ("style", "urn:oasis:names:tc:opendocument:xmlns:style:1.0"),
    ("text", "urn:oasis:names:tc:opendocument:xmlns:text:1.0"),
    ("table", "urn:oasis:names:tc:opendocument:xmlns:table:1.0"),
    ("draw", "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"),
    (
        "fo",
        "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0",
    ),
    ("xlink", "http://www.w3.org/1999/xlink"),
    ("dc", "http://purl.org/dc/elements/1.1/"),
    ("meta", "urn:oasis:names:tc:opendocument:xmlns:meta:1.0"),
    (
        "number",
        "urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0",
    ),
    (
        "svg",
        "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0",
    ),
    ("xml", "http://www.w3.org/XML/1998/namespace"),
];

/// A part's root element with every namespace declared and the version set.
#[must_use]
pub fn root(name: &str) -> Node {
    let mut node = Node::new(name);
    for (prefix, uri) in NAMESPACES {
        if prefix != "xml" {
            node = node.attr(&format!("xmlns:{prefix}"), uri);
        }
    }
    node.attr("office:version", VERSION)
}

/// The manifest namespace.
pub const MANIFEST: &str = "urn:oasis:names:tc:opendocument:xmlns:manifest:1.0";

/// Which part content lives in: the body and footnotes in `content.xml`, headers and footers in
/// `styles.xml`, each with its own automatic styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    /// `content.xml`.
    Content,
    /// `styles.xml`.
    Styles,
}

impl Part {
    /// The part's file name.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Content => "content.xml",
            Self::Styles => "styles.xml",
        }
    }
}

//! Losses recorded as an element or attribute path (`w:rPr/w:szCs`, `style:style@style:class`,
//! a bare `w:proofErr`), and whole parts not read.
//!
//! A path is a container and a leaf. A leaf only an editor uses is **Info** whatever holds it;
//! so is everything in a settings or properties container. Any other attribute, and any
//! element of a formatting container (a `...Pr`, a `...-properties`, a style or list
//! definition), is formatting: **Minor**. Anything else is an element of the content itself,
//! so its loss is **Major**.

use super::labels::{
    CONTENT_NOT_KEPT, EDITING_MARKS, FORMAT_NOT_KEPT, IMAGE_NOT_KEPT, PART_NOT_READ as PART,
    PROPERTIES, SCRIPTS, SETTINGS,
};
use super::{Kind, Severity};

/// The sentence a reader records for a part it does not read.
pub(super) const PART_NOT_READ: &str = "part not read";

/// Parts that hold nothing a reader sees: metadata, application state, previews, fonts and
/// themes the editor resolves its own way.
const INFO_PARTS: &[(&str, &str)] = &[
    ("docProps/", PROPERTIES),
    ("customXml/", PROPERTIES),
    ("word/fontTable.xml", SETTINGS),
    ("word/theme/", SETTINGS),
    ("word/webSettings.xml", SETTINGS),
    ("word/stylesWithEffects.xml", SETTINGS),
    ("word/people.xml", EDITING_MARKS),
    ("word/commentsExtended.xml", EDITING_MARKS),
    ("word/commentsIds.xml", EDITING_MARKS),
    ("word/commentsExtensible.xml", EDITING_MARKS),
    ("word/printerSettings/", SETTINGS),
    ("word/glossary/", SETTINGS),
    ("Thumbnails/", PROPERTIES),
    ("Configurations2/", SETTINGS),
    ("manifest.rdf", PROPERTIES),
    ("layout-cache", SETTINGS),
];

/// Leaves only an editor uses: revision ids, proofing and bookmark marks, style-gallery
/// hints, list ids.
const EDITING_LEAVES: &[&str] = &[
    "w:rsid",
    "w:rsidR",
    "w:rsidRPr",
    "w:rsidP",
    "w:rsidRDefault",
    "w:rsidDel",
    "w:rsids",
    "w:proofErr",
    "w:bookmarkStart",
    "w:bookmarkEnd",
    "w:lastRenderedPageBreak",
    "w:permStart",
    "w:permEnd",
    "w:qFormat",
    "w:uiPriority",
    "w:semiHidden",
    "w:unhideWhenUsed",
    "w:link",
    "w:locked",
    "w:personal",
    "w:personalCompose",
    "w:personalReply",
    "w:autoRedefine",
    "w:latentStyles",
    "w:nsid",
    "w:tmpl",
    "w:multiLevelType",
    "w:noProof",
    "w:webHidden",
    "w:tblLook",
    "officeooo:rsid",
    "officeooo:paragraph-rsid",
    "style:class",
    "style:auto-update",
];

/// Leaves for other scripts than the text's, and language tags.
const SCRIPT_LEAVES: &[&str] = &[
    "w:lang",
    "w:szCs",
    "w:sz-cs",
    "w:bCs",
    "w:iCs",
    "w:cs",
    "w:eastAsianLayout",
    "w:wordWrap",
    "w:kinsoku",
    "w:overflowPunct",
    "w:topLinePunct",
    "w:autoSpaceDE",
    "w:autoSpaceDN",
    "w:docGrid",
    "w:snapToGrid",
    "fo:language",
    "fo:country",
    "fo:script",
    "style:line-break",
    "style:punctuation-wrap",
    "style:text-autospace",
    "style:font-pitch",
    "style:font-family-generic",
    "style:letter-kerning",
    "style:use-window-font-color",
];

/// Containers whose every child is metadata or application state.
const INFO_CONTAINERS: &[(&str, &str)] = &[
    ("w:settings", SETTINGS),
    ("cp:coreProperties", PROPERTIES),
    ("office:meta", PROPERTIES),
    ("office:settings", SETTINGS),
];

/// The kind of "part not read" for `part`.
pub(super) fn part_not_read(part: &str) -> Kind {
    let part = part.trim_start_matches('/');
    INFO_PARTS
        .iter()
        .find(|(prefix, _)| part.starts_with(prefix))
        .map_or(Kind::new(Severity::Major, PART), |&(_, label)| {
            Kind::new(Severity::Info, label)
        })
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.contains(':') && !s.contains(char::is_whitespace)
}

/// Whether `container` holds formatting rather than content.
fn formats(container: &str) -> bool {
    let local = container.rsplit(':').next().unwrap_or(container);
    local.ends_with("Pr")
        || local.ends_with("-properties")
        || local.ends_with("-style")
        || local.ends_with("Bdr")
        || matches!(
            container,
            "w:style"
                | "w:styles"
                | "w:lvl"
                | "w:abstractNum"
                | "w:num"
                | "w:numbering"
                | "w:tblStylePr"
                | "w:tblGrid"
                | "w:tcBorders"
                | "w:tblBorders"
                | "style:style"
                | "style:default-style"
                | "style:master-page"
                | "style:page-layout"
                | "office:styles"
                | "office:automatic-styles"
                | "text:list-style"
                | "text:list-level-style-number"
                | "text:list-level-style-bullet"
        )
}

/// The kind of an element or attribute path, or `None` when `what` is not one.
pub(super) fn path(what: &str) -> Option<Kind> {
    if !is_name(what) {
        return None;
    }
    let cut = what.rfind(['/', '@']);
    let (container, leaf) = cut.map_or(("", what), |i| (&what[..i], &what[i + 1..]));
    let attribute = cut.is_some_and(|i| what[i..].starts_with('@'));
    let outer = container.split(['/', '@']).next().unwrap_or(container);
    let kind = if EDITING_LEAVES.contains(&leaf) {
        Kind::new(Severity::Info, EDITING_MARKS)
    } else if SCRIPT_LEAVES.contains(&leaf)
        || leaf.ends_with("-asian")
        || leaf.ends_with("-complex")
        || leaf.starts_with("style:layout-grid")
    {
        Kind::new(Severity::Info, SCRIPTS)
    } else if let Some(&(_, label)) = INFO_CONTAINERS.iter().find(|(c, _)| *c == outer) {
        Kind::new(Severity::Info, label)
    } else if attribute || formats(container) {
        Kind::new(Severity::Minor, FORMAT_NOT_KEPT)
    } else if leaf.starts_with("draw:") || leaf.starts_with("w:pict") || leaf == "w:object" {
        Kind::new(Severity::Major, IMAGE_NOT_KEPT)
    } else {
        Kind::new(Severity::Major, CONTENT_NOT_KEPT)
    };
    Some(kind)
}

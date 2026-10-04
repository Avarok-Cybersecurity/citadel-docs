//! `word/settings.xml` and `docProps/core.xml`.

use crate::props::{length, twips};
use crate::report::Log;
use crate::xml::{on_off, Node};
use cdoc_model::{DocumentMeta, DocumentSettings, Length};

/// Footnote separator ids the writer reserves; user footnotes use their own ids.
pub const SEPARATOR_IDS: [i32; 2] = [-1, -2];

/// `w:settings`.
#[must_use]
pub fn write_settings(settings: &DocumentSettings, has_footnotes: bool, log: &mut Log) -> Node {
    let mut node = Node::new("w:settings");
    if settings.track_changes {
        node = node.child(on_off("w:trackRevisions", true));
    }
    node = node.child(Node::val(
        "w:defaultTabStop",
        &twips(settings.default_tab_stop, log).to_string(),
    ));
    if settings.even_and_odd_headers {
        node = node.child(on_off("w:evenAndOddHeaders", true));
    }
    if has_footnotes {
        node = node.child(
            Node::new("w:footnotePr")
                .children(SEPARATOR_IDS.map(|id| Node::new("w:footnote").num("w:id", id))),
        );
    }
    node
}

/// Read `w:settings`.
pub fn read_settings(node: &Node, log: &mut Log) -> DocumentSettings {
    let mut s = DocumentSettings {
        even_and_odd_headers: false,
        track_changes: false,
        default_tab_stop: Length::from_twips(720),
    };
    for child in node.elems() {
        match child.name.as_str() {
            "w:trackRevisions" => s.track_changes = child.is_on(),
            "w:evenAndOddHeaders" => s.even_and_odd_headers = child.is_on(),
            "w:defaultTabStop" => {
                if let Some(l) = length(child, "w:val", log) {
                    s.default_tab_stop = l;
                }
            }
            "w:footnotePr" => {
                if child.elems().any(|n| n.name != "w:footnote") {
                    log.lose("w:footnotePr formatting");
                }
            }
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    s
}

const CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const DC: &str = "http://purl.org/dc/elements/1.1/";

/// `docProps/core.xml`, or `None` when there is no metadata.
#[must_use]
pub fn write_core(meta: &DocumentMeta) -> Option<Node> {
    if meta.title.is_none() && meta.language.is_none() {
        return None;
    }
    let mut node = Node::new("cp:coreProperties")
        .attr("xmlns:cp", CP)
        .attr("xmlns:dc", DC);
    if let Some(title) = &meta.title {
        node = node.child(Node::new("dc:title").text(title));
    }
    if let Some(language) = &meta.language {
        node = node.child(Node::new("dc:language").text(language));
    }
    Some(node)
}

/// Read `docProps/core.xml`. Other properties (author, dates) are not modelled and are noted.
pub fn read_core(node: &Node, log: &mut Log) -> DocumentMeta {
    let mut meta = DocumentMeta {
        title: None,
        language: None,
    };
    for child in node.elems() {
        match child.name.as_str() {
            "dc:title" => meta.title = Some(child.content()),
            "dc:language" => meta.language = Some(child.content()),
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    meta
}

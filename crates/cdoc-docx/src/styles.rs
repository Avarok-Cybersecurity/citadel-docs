//! `word/styles.xml`: document defaults and named styles. The cascade itself lives in the
//! model; this only moves the definitions.

use crate::props::{read_ppr, read_rpr, style_kind_from, style_kind_name, write_ppr, write_rpr};
use crate::report::Log;
use crate::xml::Node;
use cdoc_model::{
    DocumentDefaults, ParagraphProperties, RunProperties, Style, StyleId, StyleSheet,
};

/// `w:styles`.
#[must_use]
pub fn write_styles(sheet: &StyleSheet, log: &mut Log) -> Node {
    let defaults = Node::new("w:docDefaults")
        .child(Node::new("w:rPrDefault").child(write_rpr(&sheet.defaults.run)))
        .child(Node::new("w:pPrDefault").child(write_ppr(&sheet.defaults.paragraph, log)));
    let styles: Vec<Node> = sheet.styles.iter().map(|s| write_style(s, log)).collect();
    Node::new("w:styles").child(defaults).children(styles)
}

fn write_style(style: &Style, log: &mut Log) -> Node {
    let mut node = Node::new("w:style")
        .attr("w:type", style_kind_name(style.kind))
        .attr("w:styleId", style.id.as_str())
        .child(Node::val("w:name", &style.name));
    if let Some(base) = &style.based_on {
        node = node.child(Node::val("w:basedOn", base.as_str()));
    }
    if let Some(next) = &style.next {
        node = node.child(Node::val("w:next", next.as_str()));
    }
    if !style.paragraph.is_none() {
        node = node.child(write_ppr(&style.paragraph, log));
    }
    if !style.run.is_none() {
        node = node.child(write_rpr(&style.run));
    }
    node
}

/// Read `w:styles`. The document language, if the defaults carry one, is returned beside the
/// sheet because the model keeps it in the metadata.
pub fn read_styles(root: &Node, log: &mut Log) -> (StyleSheet, Option<String>) {
    let mut defaults = DocumentDefaults {
        run: RunProperties::none(),
        paragraph: ParagraphProperties::none(),
    };
    let mut language = None;
    let mut styles = Vec::new();
    for child in root.elems() {
        match child.name.as_str() {
            "w:docDefaults" => language = read_defaults(child, &mut defaults, log),
            "w:style" => styles.extend(read_style(child, log)),
            other => log.unknown(&format!("{}/{other}", root.name)),
        }
    }
    (StyleSheet { defaults, styles }, language)
}

fn read_defaults(node: &Node, out: &mut DocumentDefaults, log: &mut Log) -> Option<String> {
    let mut language = None;
    for part in node.elems() {
        match part.name.as_str() {
            "w:rPrDefault" => {
                if let Some(rpr) = part.find("w:rPr") {
                    let mut rpr = rpr.clone();
                    if let Some(at) = rpr
                        .children
                        .iter()
                        .position(|c| matches!(c, crate::xml::Child::Node(n) if n.name == "w:lang"))
                    {
                        if let crate::xml::Child::Node(lang) = rpr.children.remove(at) {
                            language = lang.value().map(str::to_owned);
                        }
                    }
                    out.run = read_rpr(&rpr, log);
                }
            }
            "w:pPrDefault" => {
                if let Some(ppr) = part.find("w:pPr") {
                    out.paragraph = read_ppr(ppr, log);
                }
            }
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    language
}

fn read_style(node: &Node, log: &mut Log) -> Option<Style> {
    let kind = node.get("w:type").unwrap_or("paragraph");
    let Some(kind) = style_kind_from(kind) else {
        log.lose(&format!("w:style of type {kind}"));
        return None;
    };
    let mut style = Style {
        id: StyleId::new(node.get("w:styleId")?),
        name: String::new(),
        kind,
        based_on: None,
        next: None,
        paragraph: ParagraphProperties::none(),
        run: RunProperties::none(),
    };
    for child in node.elems() {
        match child.name.as_str() {
            "w:name" => child
                .value()
                .unwrap_or_default()
                .clone_into(&mut style.name),
            "w:basedOn" => style.based_on = child.value().map(StyleId::new),
            "w:next" => style.next = child.value().map(StyleId::new),
            "w:pPr" => style.paragraph = read_ppr(child, log),
            "w:rPr" => style.run = read_rpr(child, log),
            other => log.unknown(&format!("w:style/{other}")),
        }
    }
    Some(style)
}

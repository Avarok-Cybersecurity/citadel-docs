//! Named styles and document defaults: `style:style` in `office:styles` and the paragraph
//! family's `style:default-style`. A style id becomes the ODF style name, escaped the way
//! `LibreOffice` escapes names (`_20_` for a space) when it is not an XML name; the display
//! name is the model's name.

use crate::props::{read_paragraph, read_run, write_paragraph, write_run};
use crate::props::{PARAGRAPH_PROPERTIES, TEXT_PROPERTIES};
use crate::units;
use cdoc_model::{
    DocumentDefaults, Length, ParagraphProperties, RunProperties, Style, StyleId, StyleKind,
    StyleSheet,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;
use std::collections::{BTreeMap, BTreeSet};

/// The ODF family of a style kind.
#[must_use]
pub const fn family(kind: StyleKind) -> &'static str {
    match kind {
        StyleKind::Paragraph => "paragraph",
        StyleKind::Character => "text",
        StyleKind::Table => "table",
    }
}

fn name_char(c: char, first: bool) -> bool {
    c.is_alphabetic() || c == '_' || (!first && (c.is_numeric() || c == '-' || c == '.'))
}

fn escape(id: &str) -> String {
    let mut out = String::new();
    for (i, c) in id.chars().enumerate() {
        if name_char(c, i == 0) {
            out.push(c);
        } else {
            let hex = format!("{:x}", u32::from(c));
            out.push('_');
            out.push_str(&hex);
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// The ODF name of every style id, unique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleNames {
    names: BTreeMap<StyleId, String>,
}

impl StyleNames {
    /// Names for the sheet's styles: the id itself when it is an XML name, else escaped, with a
    /// numeric suffix if two ids would collide.
    #[must_use]
    pub fn for_sheet(sheet: &StyleSheet) -> Self {
        let mut taken = BTreeSet::new();
        let mut names = BTreeMap::new();
        for style in &sheet.styles {
            if names.contains_key(&style.id) {
                continue;
            }
            let base = escape(style.id.as_str());
            let mut name = base.clone();
            let mut n = 2;
            while !taken.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            names.insert(style.id.clone(), name);
        }
        Self { names }
    }

    /// The ODF name for an id (an undefined one is escaped on the fly).
    #[must_use]
    pub fn of(&self, id: &StyleId) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| escape(id.as_str()))
    }
}

/// A named style. `list` gives the list style name for a style's list membership.
#[must_use]
pub fn write_style(style: &Style, names: &StyleNames, list: Option<String>) -> Node {
    let odf = names.of(&style.id);
    let mut node = Node::new("style:style").attr("style:name", &odf);
    if style.name != odf {
        node = node.attr("style:display-name", &style.name);
    }
    node = node.attr("style:family", family(style.kind));
    if let Some(base) = &style.based_on {
        node = node.attr("style:parent-style-name", &names.of(base));
    }
    if let Some(next) = &style.next {
        node = node.attr("style:next-style-name", &names.of(next));
    }
    match style.kind {
        StyleKind::Paragraph => {
            if let Some(level) = style.paragraph.outline_level {
                node = node.num("style:default-outline-level", u32::from(level) + 1);
            }
            if let Some(list) = list {
                node = node.attr("style:list-style-name", &list);
            }
            node.child(write_paragraph(&style.paragraph))
                .child(write_run(&style.run))
        }
        StyleKind::Character => node.child(write_run(&style.run)),
        StyleKind::Table => node,
    }
}

const STYLE_ATTRS: [&str; 6] = [
    "style:name",
    "style:display-name",
    "style:family",
    "style:parent-style-name",
    "style:next-style-name",
    "style:default-outline-level",
];

/// Read a named style of one of the model's families; `None` for any other family.
pub fn read_style(node: &Node, faces: &BTreeMap<String, String>, log: &mut Log) -> Option<Style> {
    let kind = match node.get("style:family")? {
        "paragraph" => StyleKind::Paragraph,
        "text" => StyleKind::Character,
        "table" => StyleKind::Table,
        _ => return None,
    };
    let id = node.get("style:name")?.to_owned();
    let mut paragraph = node
        .find(PARAGRAPH_PROPERTIES)
        .map_or_else(ParagraphProperties::none, |p| {
            read_paragraph(p, &["style:page-number"], log)
        });
    paragraph.outline_level = node
        .get("style:default-outline-level")
        .and_then(|l| l.parse::<u8>().ok())
        .filter(|l| (1..=9).contains(l))
        .map(|l| l - 1);
    let run = node
        .find(TEXT_PROPERTIES)
        .map_or_else(RunProperties::none, |t| read_run(t, faces, log));
    for (key, _) in &node.attrs {
        match key.as_str() {
            k if STYLE_ATTRS.contains(&k) => {}
            // The caller reports a list the style's residual does not restore.
            "style:master-page-name" | "style:list-style-name" => {}
            other => log.lose(&format!("style:style@{other}")),
        }
    }
    for child in node.elems() {
        if child.name != PARAGRAPH_PROPERTIES && child.name != TEXT_PROPERTIES {
            log.lose(&format!("style:style/{}", child.name));
        }
    }
    Some(Style {
        name: node.get("style:display-name").unwrap_or(&id).to_owned(),
        id: StyleId::new(id),
        kind,
        based_on: node.get("style:parent-style-name").map(StyleId::new),
        next: node.get("style:next-style-name").map(StyleId::new),
        paragraph,
        run,
    })
}

/// The paragraph family's default style: document defaults and the default tab stop.
#[must_use]
pub fn write_defaults(defaults: &DocumentDefaults, tab_stop: Length) -> Node {
    let paragraph = write_paragraph(&defaults.paragraph)
        .attr("style:tab-stop-distance", &units::write_length(tab_stop.0));
    Node::new("style:default-style")
        .attr("style:family", "paragraph")
        .child(paragraph)
        .child(write_run(&defaults.run))
}

/// Read the paragraph family's default style: the defaults and the default tab stop.
pub fn read_defaults(
    node: &Node,
    faces: &BTreeMap<String, String>,
    log: &mut Log,
) -> (DocumentDefaults, Option<Length>) {
    let p = node.find(PARAGRAPH_PROPERTIES);
    let paragraph = p.map_or_else(ParagraphProperties::none, |p| {
        read_paragraph(p, &["style:tab-stop-distance"], log)
    });
    let tab = p
        .and_then(|p| p.get("style:tab-stop-distance"))
        .and_then(units::length)
        .map(Length);
    let run = node
        .find(TEXT_PROPERTIES)
        .map_or_else(RunProperties::none, |t| read_run(t, faces, log));
    for child in node.elems() {
        if child.name != PARAGRAPH_PROPERTIES && child.name != TEXT_PROPERTIES {
            log.lose(&format!("style:default-style/{}", child.name));
        }
    }
    (DocumentDefaults { run, paragraph }, tab)
}

#[cfg(test)]
#[path = "styles_tests.rs"]
mod tests;

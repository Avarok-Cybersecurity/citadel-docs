//! The ProseMirror schema implied by the mapping, as data. Node and mark names come from
//! [`crate::names`]; attribute names come from the model types themselves (the serde field
//! names of a fully populated instance, see `schema/full.rs`). The UI compares its TipTap schema to
//! `schema_json()` in a test, so a renamed field or a new node fails CI on both sides.

mod check;
mod full;

pub use check::check;

use crate::names::{attr, mark, node};
use serde::{Deserialize, Serialize};

/// A node type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSpec {
    /// Type name.
    pub name: String,
    /// ProseMirror content expression; empty for a leaf.
    pub content: String,
    /// Group membership: `block` or `inline`, or none for structural nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Whether the node is inline.
    pub inline: bool,
    /// Whether the node is an atom (no editable content).
    pub atom: bool,
    /// Attribute names.
    pub attrs: Vec<String>,
    /// The ProseMirror `marks` spec: `_` for all, a space-separated list, or empty for none.
    pub marks: String,
    /// Child node types the content expression admits; what [`check`] enforces.
    pub children: Vec<String>,
}

/// A mark type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkSpec {
    /// Type name.
    pub name: String,
    /// Attribute names.
    pub attrs: Vec<String>,
    /// ProseMirror `excludes`: which marks this one cannot coexist with. `None` is the
    /// default (excludes itself); empty allows several instances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excludes: Option<String>,
}

/// The schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSpec {
    /// The root node type.
    pub top_node: String,
    /// Nodes, root first.
    pub nodes: Vec<NodeSpec>,
    /// Marks, in rank order (the order ProseMirror keeps them on a node).
    pub marks: Vec<MarkSpec>,
}

impl SchemaSpec {
    /// Look up a node type.
    #[must_use]
    pub fn node(&self, name: &str) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Look up a mark type.
    #[must_use]
    pub fn mark(&self, name: &str) -> Option<&MarkSpec> {
        self.marks.iter().find(|m| m.name == name)
    }
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn structural(name: &str, content: &str, attrs: Vec<String>, children: &[&str]) -> NodeSpec {
    NodeSpec {
        name: name.to_owned(),
        content: content.to_owned(),
        group: None,
        inline: false,
        atom: false,
        attrs,
        marks: String::new(),
        children: strs(children),
    }
}

fn block(name: &str, content: &str, attrs: Vec<String>, children: &[&str]) -> NodeSpec {
    NodeSpec {
        group: Some("block".to_owned()),
        ..structural(name, content, attrs, children)
    }
}

const BLOCKS: &[&str] = &[node::PARAGRAPH, node::TABLE];
const INLINES: &[&str] = &[
    node::TEXT,
    node::IMAGE,
    node::HARD_BREAK,
    node::PAGE_BREAK,
    node::COLUMN_BREAK,
    node::TAB,
    node::FOOTNOTE_REF,
    node::FIELD,
    node::FORM_FIELD,
    node::SIGNATURE_FIELD,
];
const RANGE_MARKS: &str = "link comment insertion deletion";

fn inline_atom(name: &str, attrs: Vec<String>) -> NodeSpec {
    NodeSpec {
        name: name.to_owned(),
        content: String::new(),
        group: Some("inline".to_owned()),
        inline: true,
        atom: true,
        attrs,
        marks: RANGE_MARKS.to_owned(),
        children: Vec::new(),
    }
}

fn mark_spec(name: &str, attrs: Vec<String>, excludes: Option<&str>) -> MarkSpec {
    MarkSpec {
        name: name.to_owned(),
        attrs,
        excludes: excludes.map(str::to_owned),
    }
}

/// The schema.
#[must_use]
pub fn schema() -> SchemaSpec {
    let cells = &[node::TABLE_CELL, node::TABLE_HEADER];
    let nodes = vec![
        structural(
            node::DOC,
            "section+ footnotes?",
            strs(&[
                attr::META,
                attr::SETTINGS,
                attr::STYLES,
                attr::NUMBERING,
                attr::COMMENTS,
            ]),
            &[node::SECTION, node::FOOTNOTES],
        ),
        structural(
            node::SECTION,
            "(header | footer)* block+",
            full::section_attrs(),
            &[node::HEADER, node::FOOTER, node::PARAGRAPH, node::TABLE],
        ),
        structural(node::HEADER, "block+", strs(&[attr::KIND]), BLOCKS),
        structural(node::FOOTER, "block+", strs(&[attr::KIND]), BLOCKS),
        block(node::PARAGRAPH, "inline*", full::paragraph_attrs(), INLINES),
        block(
            node::TABLE,
            "tableRow+",
            full::table_attrs(),
            &[node::TABLE_ROW],
        ),
        structural(
            node::TABLE_ROW,
            "(tableCell | tableHeader)+",
            full::row_attrs(),
            cells,
        ),
        structural(node::TABLE_CELL, "block+", full::cell_attrs(), BLOCKS),
        structural(node::TABLE_HEADER, "block+", full::cell_attrs(), BLOCKS),
        NodeSpec {
            marks: "_".to_owned(),
            atom: false,
            ..inline_atom(node::TEXT, Vec::new())
        },
        inline_atom(node::IMAGE, full::image_attrs()),
        inline_atom(node::HARD_BREAK, Vec::new()),
        inline_atom(node::PAGE_BREAK, Vec::new()),
        inline_atom(node::COLUMN_BREAK, Vec::new()),
        inline_atom(node::TAB, Vec::new()),
        inline_atom(node::FOOTNOTE_REF, strs(&[attr::ID])),
        inline_atom(node::FIELD, full::field_attrs()),
        inline_atom(node::FORM_FIELD, full::form_field_attrs()),
        inline_atom(node::SIGNATURE_FIELD, full::signature_field_attrs()),
        structural(node::FOOTNOTES, "footnote+", Vec::new(), &[node::FOOTNOTE]),
        structural(node::FOOTNOTE, "block+", strs(&[attr::ID]), BLOCKS),
    ];
    let marks = vec![
        mark_spec(mark::CHARACTER_STYLE, strs(&[attr::STYLE_ID]), None),
        mark_spec(
            mark::TEXT_STYLE,
            strs(&[attr::FONT_FAMILY, attr::FONT_SIZE, attr::COLOR]),
            None,
        ),
        mark_spec(mark::BOLD, strs(&[attr::ON]), None),
        mark_spec(mark::ITALIC, strs(&[attr::ON]), None),
        mark_spec(mark::UNDERLINE, full::underline_attrs(), None),
        mark_spec(mark::STRIKE, strs(&[attr::ON]), None),
        mark_spec(mark::VERTICAL_ALIGN, strs(&[attr::VALUE]), None),
        mark_spec(mark::HIGHLIGHT, strs(&[attr::COLOR]), None),
        mark_spec(mark::SMALL_CAPS, strs(&[attr::ON]), None),
        mark_spec(mark::ALL_CAPS, strs(&[attr::ON]), None),
        mark_spec(mark::LINK, full::link_attrs(), None),
        mark_spec(mark::COMMENT, strs(&[attr::ID]), Some("")),
        mark_spec(
            mark::INSERTION,
            strs(&[attr::ID, "author", "date"]),
            Some("deletion"),
        ),
        mark_spec(
            mark::DELETION,
            strs(&[attr::ID, "author", "date"]),
            Some("insertion"),
        ),
    ];
    SchemaSpec {
        top_node: node::DOC.to_owned(),
        nodes,
        marks,
    }
}

/// The schema as pretty JSON, what `schema/prosemirror-schema.json` holds.
///
/// # Panics
/// Never: the schema is plain data.
#[must_use]
pub fn schema_json() -> String {
    serde_json::to_string_pretty(&schema()).expect("schema serialises")
}

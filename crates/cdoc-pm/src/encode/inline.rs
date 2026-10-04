//! Inlines → nodes with marks. Character formatting becomes marks (the editor's toolbar
//! toggles marks); range annotations (link, comments, revision) become marks too.

use super::to_value;
use crate::json::{attrs_from, put, PmMark, PmNode};
use crate::names::{attr, mark, node};
use cdoc_model::{BreakKind, Inline, InlineContent, Marks, RevisionKind, RunProperties};
use serde_json::{Map, Value};

/// An inline as a node carrying its marks.
pub(crate) fn encode_inline(inline: &Inline) -> PmNode {
    let mut node = match &inline.content {
        InlineContent::Text(run) => {
            let mut n = PmNode::new(node::TEXT);
            n.text = Some(run.text.clone());
            n.marks = run_marks(&run.properties);
            n
        }
        InlineContent::Image(image) => PmNode::with(node::IMAGE, attrs_from(image), Vec::new()),
        InlineContent::Break { kind } => PmNode::new(match kind {
            BreakKind::Line => node::HARD_BREAK,
            BreakKind::Page => node::PAGE_BREAK,
            BreakKind::Column => node::COLUMN_BREAK,
        }),
        InlineContent::Tab => PmNode::new(node::TAB),
        InlineContent::FootnoteRef { id } => {
            let mut attrs = Map::new();
            attrs.insert(attr::ID.to_owned(), to_value(id));
            PmNode::with(node::FOOTNOTE_REF, attrs, Vec::new())
        }
        InlineContent::Field(field) => PmNode::with(node::FIELD, attrs_from(field), Vec::new()),
        InlineContent::FormField(field) => {
            PmNode::with(node::FORM_FIELD, attrs_from(field), Vec::new())
        }
        InlineContent::SignatureField(field) => {
            PmNode::with(node::SIGNATURE_FIELD, attrs_from(field), Vec::new())
        }
    };
    node.marks.extend(range_marks(&inline.marks));
    node
}

fn on_mark(kind: &str, value: Option<bool>) -> Option<PmMark> {
    value.map(|on| {
        let mut attrs = Map::new();
        attrs.insert(attr::ON.to_owned(), Value::Bool(on));
        PmMark::new(kind, attrs)
    })
}

/// Character formatting as marks, in a fixed order.
pub(crate) fn run_marks(props: &RunProperties) -> Vec<PmMark> {
    let mut marks = Vec::new();
    if let Some(style) = &props.style {
        let mut attrs = Map::new();
        attrs.insert(attr::STYLE_ID.to_owned(), to_value(style));
        marks.push(PmMark::new(mark::CHARACTER_STYLE, attrs));
    }
    if props.font.is_some() || props.size.is_some() || props.color.is_some() {
        let mut attrs = Map::new();
        put(&mut attrs, attr::FONT_FAMILY, &props.font);
        put(&mut attrs, attr::FONT_SIZE, &props.size);
        put(&mut attrs, attr::COLOR, &props.color);
        marks.push(PmMark::new(mark::TEXT_STYLE, attrs));
    }
    marks.extend(on_mark(mark::BOLD, props.bold));
    marks.extend(on_mark(mark::ITALIC, props.italic));
    if let Some(underline) = &props.underline {
        marks.push(PmMark::new(mark::UNDERLINE, attrs_from(underline)));
    }
    marks.extend(on_mark(mark::STRIKE, props.strike));
    if let Some(vertical) = props.vertical_align {
        let mut attrs = Map::new();
        attrs.insert(attr::VALUE.to_owned(), to_value(&vertical));
        marks.push(PmMark::new(mark::VERTICAL_ALIGN, attrs));
    }
    if let Some(color) = props.highlight {
        let mut attrs = Map::new();
        attrs.insert(attr::COLOR.to_owned(), to_value(&color));
        marks.push(PmMark::new(mark::HIGHLIGHT, attrs));
    }
    marks.extend(on_mark(mark::SMALL_CAPS, props.small_caps));
    marks.extend(on_mark(mark::ALL_CAPS, props.all_caps));
    marks
}

/// Range annotations as marks: link, then comments in anchoring order, then the revision.
pub(crate) fn range_marks(marks: &Marks) -> Vec<PmMark> {
    let mut out = Vec::new();
    if let Some(link) = &marks.link {
        out.push(PmMark::new(mark::LINK, attrs_from(link)));
    }
    for id in &marks.comments {
        let mut attrs = Map::new();
        attrs.insert(attr::ID.to_owned(), to_value(id));
        out.push(PmMark::new(mark::COMMENT, attrs));
    }
    if let Some(revision) = &marks.revision {
        let mut attrs = attrs_from(revision);
        attrs.remove("kind");
        let kind = match revision.kind {
            RevisionKind::Insertion => mark::INSERTION,
            RevisionKind::Deletion => mark::DELETION,
        };
        out.push(PmMark::new(kind, attrs));
    }
    out
}

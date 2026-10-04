//! Paragraphs and tables → nodes.

use super::{encode_inline, to_value};
use crate::json::{attrs_from, PmNode};
use crate::names::{attr, node};
use cdoc_model::{Block, Paragraph, Table, TableCell, TableRow};
use serde_json::Map;

/// Blocks in order.
pub(crate) fn encode_blocks(blocks: &[Block]) -> Vec<PmNode> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => encode_paragraph(p),
            Block::Table(t) => encode_table(t),
        })
        .collect()
}

/// A paragraph: its properties are its attributes, one attribute per field.
pub(crate) fn encode_paragraph(paragraph: &Paragraph) -> PmNode {
    PmNode::with(
        node::PARAGRAPH,
        attrs_from(&paragraph.properties),
        paragraph.inlines.iter().map(encode_inline).collect(),
    )
}

/// A table: properties plus `columnWidths` as attributes, rows as children.
pub(crate) fn encode_table(table: &Table) -> PmNode {
    let mut attrs = attrs_from(&table.properties);
    attrs.insert(
        attr::COLUMN_WIDTHS.to_owned(),
        to_value(&table.column_widths),
    );
    PmNode::with(
        node::TABLE,
        attrs,
        table.rows.iter().map(encode_row).collect(),
    )
}

fn encode_row(row: &TableRow) -> PmNode {
    let mut attrs = attrs_from(row);
    attrs.remove("cells");
    PmNode::with(
        node::TABLE_ROW,
        attrs,
        row.cells.iter().map(encode_cell).collect(),
    )
}

/// A cell: `tableHeader` or `tableCell`, with spans spelled the way `prosemirror-tables`
/// reads them.
fn encode_cell(cell: &TableCell) -> PmNode {
    let mut attrs: Map<String, serde_json::Value> = attrs_from(&cell.properties);
    let colspan = attrs.remove("colSpan").expect("colSpan is not optional");
    let rowspan = attrs.remove("rowSpan").expect("rowSpan is not optional");
    attrs.insert(attr::COLSPAN.to_owned(), colspan);
    attrs.insert(attr::ROWSPAN.to_owned(), rowspan);
    let kind = if cell.header {
        node::TABLE_HEADER
    } else {
        node::TABLE_CELL
    };
    PmNode::with(kind, attrs, encode_blocks(&cell.blocks))
}

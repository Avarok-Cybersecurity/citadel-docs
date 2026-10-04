//! Block nodes → paragraphs and tables.

use super::{decode_inline, expect_kind};
use crate::json::{attr, attrs_into, child_path, PmError, PmNode};
use crate::names::{attr as attr_name, node};
use cdoc_model::{Block, CellProperties, Paragraph, Table, TableCell, TableRow};
use serde_json::Value;

/// Blocks in order; anything other than a paragraph or table is an error.
pub(crate) fn decode_blocks(nodes: &[PmNode], path: &str) -> Result<Vec<Block>, PmError> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, n)| match n.kind.as_str() {
            node::PARAGRAPH => {
                decode_paragraph(n, &child_path(path, node::PARAGRAPH, i)).map(Block::Paragraph)
            }
            node::TABLE => decode_table(n, &child_path(path, node::TABLE, i)).map(Block::Table),
            other => Err(PmError::new(
                &child_path(path, other, i),
                format!("{other:?} is not a block"),
            )),
        })
        .collect()
}

fn decode_paragraph(node: &PmNode, path: &str) -> Result<Paragraph, PmError> {
    Ok(Paragraph {
        properties: attrs_into(&node.attrs, path)?,
        inlines: node
            .content
            .iter()
            .enumerate()
            .map(|(i, n)| decode_inline(n, &child_path(path, &n.kind, i)))
            .collect::<Result<_, _>>()?,
    })
}

fn decode_table(node: &PmNode, path: &str) -> Result<Table, PmError> {
    let column_widths = attr(&node.attrs, attr_name::COLUMN_WIDTHS, path)?;
    let mut attrs = node.attrs.clone();
    attrs.remove(attr_name::COLUMN_WIDTHS);
    let rows = node
        .content
        .iter()
        .enumerate()
        .map(|(i, n)| decode_row(n, &child_path(path, node::TABLE_ROW, i)))
        .collect::<Result<_, _>>()?;
    Ok(Table {
        properties: attrs_into(&attrs, path)?,
        column_widths,
        rows,
    })
}

fn decode_row(node: &PmNode, path: &str) -> Result<TableRow, PmError> {
    expect_kind(node, node::TABLE_ROW, path)?;
    let mut attrs = node.attrs.clone();
    attrs.insert("cells".to_owned(), Value::Array(Vec::new()));
    let mut row: TableRow = attrs_into(&attrs, path)?;
    row.cells = node
        .content
        .iter()
        .enumerate()
        .map(|(i, n)| decode_cell(n, &child_path(path, &n.kind, i)))
        .collect::<Result<_, _>>()?;
    Ok(row)
}

fn decode_cell(node: &PmNode, path: &str) -> Result<TableCell, PmError> {
    let header = match node.kind.as_str() {
        node::TABLE_HEADER => true,
        node::TABLE_CELL => false,
        other => {
            return Err(PmError::new(
                path,
                format!("expected a table cell, found {other:?}"),
            ))
        }
    };
    let mut attrs = node.attrs.clone();
    let colspan = attr::<u32>(&attrs, attr_name::COLSPAN, path)?;
    let rowspan = attr::<u32>(&attrs, attr_name::ROWSPAN, path)?;
    attrs.remove(attr_name::COLSPAN);
    attrs.remove(attr_name::ROWSPAN);
    attrs.insert("colSpan".to_owned(), Value::from(colspan));
    attrs.insert("rowSpan".to_owned(), Value::from(rowspan));
    let properties: CellProperties = attrs_into(&attrs, path)?;
    Ok(TableCell {
        properties,
        header,
        blocks: decode_blocks(&node.content, path)?,
    })
}

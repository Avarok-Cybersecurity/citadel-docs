//! Strategies for paragraphs and tables. A table cell holds paragraphs only, so recursion is
//! bounded without a depth parameter; nested tables are covered by the kitchen-sink fixture.

use super::inline::inline;
use super::props::{borders, paragraph_properties, shading, ALIGNMENTS};
use super::units::{length, positive_length, style_id, table_width};
use super::{opt, pick};
use crate::block::Block;
use crate::paragraph::Paragraph;
use crate::table::{
    CellProperties, RowHeight, Table, TableCell, TableProperties, TableRow, VerticalCellAlign,
};
use proptest::prelude::*;

/// A paragraph with up to four inlines (possibly none).
pub fn paragraph() -> BoxedStrategy<Paragraph> {
    (
        paragraph_properties(),
        prop::collection::vec(inline(), 0..=4),
    )
        .prop_map(|(properties, inlines)| Paragraph {
            properties,
            inlines,
        })
        .boxed()
}

fn row_height() -> BoxedStrategy<RowHeight> {
    prop_oneof![
        positive_length().prop_map(|height| RowHeight::AtLeast { height }),
        positive_length().prop_map(|height| RowHeight::Exact { height }),
    ]
    .boxed()
}

fn cell_properties() -> BoxedStrategy<CellProperties> {
    (
        opt(table_width()),
        1u32..=2,
        1u32..=2,
        opt(borders()),
        opt(shading()),
        opt(pick(&[
            VerticalCellAlign::Top,
            VerticalCellAlign::Center,
            VerticalCellAlign::Bottom,
        ])),
    )
        .prop_map(
            |(width, col_span, row_span, borders, shading, vertical_align)| CellProperties {
                width,
                col_span,
                row_span,
                borders,
                shading,
                vertical_align,
            },
        )
        .boxed()
}

fn cell() -> BoxedStrategy<TableCell> {
    (
        cell_properties(),
        any::<bool>(),
        prop::collection::vec(paragraph().prop_map(Block::Paragraph), 1..=2),
    )
        .prop_map(|(properties, header, blocks)| TableCell {
            properties,
            header,
            blocks,
        })
        .boxed()
}

fn row() -> BoxedStrategy<TableRow> {
    (
        opt(row_height()),
        any::<bool>(),
        prop::collection::vec(cell(), 1..=2),
    )
        .prop_map(|(height, cant_split, cells)| TableRow {
            height,
            cant_split,
            cells,
        })
        .boxed()
}

fn table_properties() -> BoxedStrategy<TableProperties> {
    (
        opt(style_id()),
        table_width(),
        opt(pick(ALIGNMENTS)),
        opt(borders()),
        opt(length()),
    )
        .prop_map(
            |(style, width, alignment, borders, cell_padding)| TableProperties {
                style,
                width,
                alignment,
                borders,
                cell_padding,
            },
        )
        .boxed()
}

/// A table with one to two rows of one to two cells, and a grid wide enough for any spans.
pub fn table() -> BoxedStrategy<Table> {
    (
        table_properties(),
        prop::collection::vec(positive_length(), 4..=4),
        prop::collection::vec(row(), 1..=2),
    )
        .prop_map(|(properties, column_widths, rows)| Table {
            properties,
            column_widths,
            rows,
        })
        .boxed()
}

/// A block: mostly paragraphs, sometimes a table.
pub fn block() -> BoxedStrategy<Block> {
    prop_oneof![
        4 => paragraph().prop_map(Block::Paragraph),
        1 => table().prop_map(Block::Table),
    ]
    .boxed()
}

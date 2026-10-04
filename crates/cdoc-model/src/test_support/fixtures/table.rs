//! The kitchen sink's table: a merged header row, a fixed-height row and a nested table.

use super::{para, text};
use crate::table::{
    CellProperties, RowHeight, Table, TableCell, TableProperties, TableRow, VerticalCellAlign,
};
use crate::units::{Color, Length, TableWidth};
use crate::{Alignment, Block};

fn cell(blocks: Vec<Block>, col_span: u32, header: bool) -> TableCell {
    TableCell {
        properties: CellProperties {
            col_span,
            vertical_align: Some(VerticalCellAlign::Center),
            ..CellProperties::single()
        },
        header,
        blocks,
    }
}

pub(super) fn table() -> Block {
    let inner = Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length::from_twips(2000)],
        rows: vec![TableRow {
            height: None,
            cant_split: false,
            cells: vec![cell(vec![para(None, vec![text("nested")])], 1, false)],
        }],
    };
    Block::Table(Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Percent { fiftieths: 5000 },
            alignment: Some(Alignment::Center),
            borders: Some(crate::Borders::all(crate::Border {
                style: crate::BorderStyle::Single,
                width: crate::BorderWidth(4),
                color: Some(Color::rgb(0, 0, 0)),
                space: Length(0),
            })),
            cell_padding: Some(Length::from_twips(108)),
        },
        column_widths: vec![Length::from_twips(4000), Length::from_twips(4000)],
        rows: vec![
            TableRow {
                height: Some(RowHeight::AtLeast {
                    height: Length::from_twips(400),
                }),
                cant_split: true,
                cells: vec![cell(vec![para(None, vec![text("Merged header")])], 2, true)],
            },
            TableRow {
                height: Some(RowHeight::Exact {
                    height: Length::from_twips(600),
                }),
                cant_split: false,
                cells: vec![
                    cell(vec![para(None, vec![text("a")])], 1, false),
                    cell(vec![Block::Table(inner)], 1, false),
                ],
            },
        ],
    })
}

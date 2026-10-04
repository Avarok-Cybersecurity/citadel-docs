//! Tables. Merged cells are expressed as `col_span` / `row_span` on the top-left cell, the way
//! `ProseMirror` and HTML do; the `.docx` writer converts to `gridSpan` / `vMerge`.

use crate::block::Block;
use crate::borders::{Borders, Shading};
use crate::ids::StyleId;
use crate::paragraph::Alignment;
use crate::units::{Length, TableWidth};
use serde::{Deserialize, Serialize};

/// Table-level properties.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableProperties {
    /// A table style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleId>,
    /// Overall width.
    pub width: TableWidth,
    /// Horizontal placement of the table in its column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Alignment>,
    /// Outer borders and the inside edges (`between` is unused for tables).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borders: Option<Borders>,
    /// Space between cell border and cell content, applied to every cell unless overridden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_padding: Option<Length>,
}

/// A row height rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "camelCase")]
pub enum RowHeight {
    /// At least this tall; grows with content.
    AtLeast {
        /// The minimum.
        height: Length,
    },
    /// Exactly this tall; content is clipped.
    Exact {
        /// The height.
        height: Length,
    },
}

/// Vertical alignment of content within a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerticalCellAlign {
    /// Top.
    Top,
    /// Middle.
    Center,
    /// Bottom.
    Bottom,
}

/// Cell-level properties.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellProperties {
    /// Preferred width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<TableWidth>,
    /// Columns spanned, `>= 1`.
    pub col_span: u32,
    /// Rows spanned, `>= 1`.
    pub row_span: u32,
    /// Borders on this cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borders: Option<Borders>,
    /// Background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<Shading>,
    /// Vertical alignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<VerticalCellAlign>,
}

impl CellProperties {
    /// A single, unmerged cell with nothing else set.
    #[must_use]
    pub const fn single() -> Self {
        Self {
            width: None,
            col_span: 1,
            row_span: 1,
            borders: None,
            shading: None,
            vertical_align: None,
        }
    }
}

/// A cell. `header` marks a header cell (rendered as `<th>`, repeated across pages by the
/// layout crate when a whole row is header cells).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCell {
    /// Cell properties.
    pub properties: CellProperties,
    /// Whether this is a header cell.
    pub header: bool,
    /// Content; never empty (a visually empty cell holds one empty paragraph).
    pub blocks: Vec<Block>,
}

/// A row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRow {
    /// Height rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<RowHeight>,
    /// Never allowed to split across a page.
    pub cant_split: bool,
    /// Cells; never empty.
    pub cells: Vec<TableCell>,
}

/// A table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    /// Table properties.
    pub properties: TableProperties,
    /// Grid column widths, one per grid column; never empty.
    pub column_widths: Vec<Length>,
    /// Rows; never empty.
    pub rows: Vec<TableRow>,
}

/// A cell at its position on the table's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement<'a> {
    /// Row index.
    pub row: usize,
    /// Index of the cell within its row.
    pub index: usize,
    /// First grid column.
    pub col: usize,
    /// Grid columns spanned, clamped to the grid.
    pub col_span: usize,
    /// Rows spanned, clamped to the table.
    pub row_span: usize,
    /// The cell.
    pub cell: &'a TableCell,
}

/// What occupies one position of a table's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot<'a> {
    /// The top-left position of a cell.
    Cell(&'a TableCell),
    /// A position another cell's column or row span covers.
    Covered,
    /// A position with no cell (a ragged row).
    Empty,
}

fn at_least_one(n: u32) -> usize {
    usize::try_from(n.max(1)).unwrap_or(1)
}

impl Table {
    /// Every cell at its grid position, in row order, skipping columns taken by row spans from
    /// above; a cell that starts past the last column is left out. This is the one place the
    /// "a span lives on its top-left cell" rule is unpacked: layout, exporters and the
    /// importers' writers all place cells with it.
    #[must_use]
    pub fn placements(&self) -> Vec<Placement<'_>> {
        let columns = self.column_widths.len().max(1);
        let rows = self.rows.len();
        let mut taken = vec![vec![false; columns]; rows];
        let mut out = Vec::new();
        for (row, model_row) in self.rows.iter().enumerate() {
            let mut col = 0;
            for (index, cell) in model_row.cells.iter().enumerate() {
                while col < columns && taken[row][col] {
                    col += 1;
                }
                if col >= columns {
                    break;
                }
                let col_span = at_least_one(cell.properties.col_span).min(columns - col);
                let row_span = at_least_one(cell.properties.row_span).min(rows - row);
                for line in taken.iter_mut().skip(row).take(row_span) {
                    for slot in line.iter_mut().skip(col).take(col_span) {
                        *slot = true;
                    }
                }
                out.push(Placement {
                    row,
                    index,
                    col,
                    col_span,
                    row_span,
                    cell,
                });
                col += col_span;
            }
        }
        out
    }

    /// The table as a rectangle, one entry per row and grid column, spans expanded.
    #[must_use]
    pub fn grid(&self) -> Vec<Vec<Slot<'_>>> {
        let columns = self.column_widths.len().max(1);
        let mut grid = vec![vec![Slot::Empty; columns]; self.rows.len()];
        for p in self.placements() {
            for line in grid.iter_mut().skip(p.row).take(p.row_span) {
                for slot in line.iter_mut().skip(p.col).take(p.col_span) {
                    *slot = Slot::Covered;
                }
            }
            grid[p.row][p.col] = Slot::Cell(p.cell);
        }
        grid
    }
}

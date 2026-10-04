//! Where each cell sits on the table's grid, which rows a row span ties together, and the
//! border rules round a cell.

use crate::ir::{Draw, Pt100, Rect};
use crate::paragraph::border_width;
use crate::style::INK;
use cdoc_model::{BorderStyle, Borders, Table, TableCell};

/// A cell at its grid position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// Row index.
    pub row: usize,
    /// Index of the cell within its row.
    pub index: usize,
    /// First grid column.
    pub col: usize,
    /// Grid columns spanned.
    pub col_span: usize,
    /// Rows spanned (clamped to the table).
    pub row_span: usize,
}

impl Placed {
    /// The model cell.
    #[must_use]
    pub fn cell<'t>(&self, table: &'t Table) -> &'t TableCell {
        &table.rows[self.row].cells[self.index]
    }

    /// Width over its grid columns.
    #[must_use]
    pub fn width(&self, widths: &[Pt100]) -> Pt100 {
        let end = (self.col + self.col_span).min(widths.len());
        widths[self.col.min(end)..end].iter().sum()
    }
}

fn to_usize(n: u32) -> usize {
    usize::try_from(n.max(1)).unwrap_or(1)
}

/// Every cell at its grid position, skipping columns taken by row spans from above.
#[must_use]
pub fn place(table: &Table) -> Vec<Placed> {
    let columns = table.column_widths.len().max(1);
    let rows = table.rows.len();
    let mut taken = vec![vec![false; columns]; rows];
    let mut out = Vec::new();
    for (row, model_row) in table.rows.iter().enumerate() {
        let mut col = 0;
        for (index, cell) in model_row.cells.iter().enumerate() {
            while col < columns && taken[row][col] {
                col += 1;
            }
            if col >= columns {
                break;
            }
            let col_span = to_usize(cell.properties.col_span).min(columns - col);
            let row_span = to_usize(cell.properties.row_span).min(rows - row);
            for line in taken.iter_mut().skip(row).take(row_span) {
                for slot in line.iter_mut().skip(col).take(col_span) {
                    *slot = true;
                }
            }
            out.push(Placed {
                row,
                index,
                col,
                col_span,
                row_span,
            });
            col += col_span;
        }
    }
    out
}

/// Runs of rows that must stay together because a row span crosses them, as
/// `(first, last)` inclusive, in order.
#[must_use]
pub fn groups(placed: &[Placed], rows: usize) -> Vec<(usize, usize)> {
    let mut reach: Vec<usize> = (0..rows).collect();
    for p in placed {
        let last = p.row + p.row_span - 1;
        reach[p.row] = reach[p.row].max(last);
    }
    let mut out = Vec::new();
    let mut row = 0;
    while row < rows {
        let mut last = reach[row];
        let mut r = row;
        while r <= last {
            last = last.max(reach[r]);
            r += 1;
        }
        out.push((row, last));
        row = last + 1;
    }
    out
}

/// The rules round a cell's rectangle.
#[must_use]
pub fn edges(borders: &Borders, rect: Rect) -> Vec<Draw> {
    let Rect {
        x,
        y,
        width,
        height,
    } = rect;
    let sides = [
        (&borders.top, (x, y), (x + width, y)),
        (&borders.bottom, (x, y + height), (x + width, y + height)),
        (&borders.left, (x, y), (x, y + height)),
        (&borders.right, (x + width, y), (x + width, y + height)),
    ];
    sides
        .into_iter()
        .filter_map(|(border, from, to)| {
            let b = border.as_ref().filter(|b| b.style != BorderStyle::None)?;
            Some(Draw::Rule {
                from,
                to,
                width: border_width(b).max(1),
                colour: b.color.unwrap_or(INK),
            })
        })
        .collect()
}

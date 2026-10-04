//! Tables: the grid scaled to the table's width, cells placed on it with their spans, rows as
//! high as their tallest cell, and rows joined by a row span kept together as one strip.

mod grid;

use crate::context::Ctx;
use crate::fragment::{Block, Fragment};
use crate::ir::{Draw, Pt100, Rect};
use crate::labels::Counters;
use crate::stack::stack;
use crate::LayoutError;
use cdoc_model::{Alignment, RowHeight, Table, TableWidth, VerticalCellAlign};
use grid::{place, Placed};

/// Grid column widths scaled to the table's width, and the table's left offset in its box.
fn columns(table: &Table, available: Pt100) -> (Vec<Pt100>, Pt100) {
    let grid: Vec<i64> = table
        .column_widths
        .iter()
        .map(|w| i64::from(w.0.max(1)))
        .collect();
    let natural: i64 = grid.iter().sum::<i64>().max(1);
    let target = match table.properties.width {
        TableWidth::Fixed { length } => i64::from(length.0),
        TableWidth::Percent { fiftieths } => i64::from(available) * i64::from(fiftieths) / 5000,
        TableWidth::Auto => natural.min(i64::from(available)),
    }
    .max(1);
    let widths: Vec<Pt100> = grid
        .iter()
        .map(|g| Pt100::try_from(g * target / natural).unwrap_or(Pt100::MAX))
        .collect();
    let total: Pt100 = widths.iter().sum();
    let offset = match table.properties.alignment {
        Some(Alignment::Center) => (available - total) / 2,
        Some(Alignment::Right) => available - total,
        _ => 0,
    };
    (widths, offset)
}

/// The table as a flow block, one strip per row (or per group of rows a row span joins).
pub fn table(
    ctx: &mut Ctx<'_, '_>,
    table: &Table,
    available: Pt100,
    counters: &mut Counters<'_>,
) -> Result<Block, LayoutError> {
    let (widths, offset) = columns(table, available);
    let padding = table
        .properties
        .cell_padding
        .unwrap_or(ctx.options.cell_padding)
        .0;
    let placed = place(table);
    let mut contents: Vec<Fragment> = Vec::with_capacity(placed.len());
    let mut heights: Vec<Pt100> = table
        .rows
        .iter()
        .map(|r| match r.height {
            Some(RowHeight::AtLeast { height } | RowHeight::Exact { height }) => height.0,
            None => 0,
        })
        .collect();
    for cell in &placed {
        let width = cell.width(&widths) - 2 * padding;
        let laid = stack(ctx, &cell.cell(table).blocks, width.max(1), counters)?;
        if cell.row_span == 1 && !exact(table, cell.row) {
            heights[cell.row] = heights[cell.row].max(laid.height);
        }
        contents.push(laid);
    }
    for (cell, laid) in placed.iter().zip(&contents) {
        let last = cell.row + cell.row_span - 1;
        let spanned: Pt100 = heights[cell.row..=last].iter().sum();
        if cell.row_span > 1 && laid.height > spanned && !exact(table, last) {
            heights[last] += laid.height - spanned;
        }
    }
    let groups = grid::groups(&placed, table.rows.len());
    let tops: Vec<Pt100> = heights
        .iter()
        .scan(0, |acc, h| {
            let top = *acc;
            *acc += h;
            Some(top)
        })
        .collect();
    let mut strips = Vec::with_capacity(groups.len());
    for &(first, last) in &groups {
        let mut strip = Fragment::empty(heights[first..=last].iter().sum());
        for (cell, laid) in placed.iter().zip(&contents) {
            if cell.row < first || cell.row > last {
                continue;
            }
            draw_cell(
                &mut strip,
                table,
                cell,
                laid.clone(),
                &widths,
                offset,
                padding,
                tops[cell.row] - tops[first],
                &heights,
            );
        }
        strips.push(strip);
    }
    let head = groups
        .iter()
        .take_while(|(f, l)| {
            table.rows[*f..=*l]
                .iter()
                .all(|r| r.cells.iter().all(|c| c.header))
        })
        .count();
    Ok(Block {
        space_before: 0,
        space_after: 0,
        strips,
        keep_together: false,
        keep_with_next: false,
        page_break_before: false,
        widow_control: false,
        repeat_head: head.min(groups.len().saturating_sub(1)),
    })
}

fn exact(table: &Table, row: usize) -> bool {
    matches!(table.rows[row].height, Some(RowHeight::Exact { .. }))
}

#[expect(
    clippy::too_many_arguments,
    reason = "one cell's geometry, passed flat"
)]
fn draw_cell(
    strip: &mut Fragment,
    table: &Table,
    cell: &Placed,
    content: Fragment,
    widths: &[Pt100],
    offset: Pt100,
    padding: Pt100,
    top: Pt100,
    heights: &[Pt100],
) {
    let x = offset + widths[..cell.col].iter().sum::<Pt100>();
    let width = cell.width(widths);
    let height: Pt100 = heights[cell.row..cell.row + cell.row_span].iter().sum();
    let model = cell.cell(table);
    if let Some(shading) = &model.properties.shading {
        strip.draws.push(Draw::Fill {
            rect: Rect {
                x,
                y: top,
                width,
                height,
            },
            colour: shading.fill,
        });
    }
    let slack = (height - content.height).max(0);
    let dy = match model.properties.vertical_align {
        Some(VerticalCellAlign::Center) => slack / 2,
        Some(VerticalCellAlign::Bottom) => slack,
        Some(VerticalCellAlign::Top) | None => 0,
    };
    strip.absorb(content, x + padding, top + dy);
    let borders = model
        .properties
        .borders
        .as_ref()
        .or(table.properties.borders.as_ref());
    if let Some(borders) = borders {
        strip.draws.extend(grid::edges(
            borders,
            Rect {
                x,
                y: top,
                width,
                height,
            },
        ));
    }
}

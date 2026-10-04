//! Tables as aligned columns.

use super::block::paragraph_text;
use super::Ctx;
use cdoc_model::{Alignment, Block, Slot, Table};

fn cell_text(blocks: &[Block], ctx: &mut Ctx<'_>) -> (String, Option<Alignment>) {
    let mut parts = Vec::new();
    let mut alignment = None;
    for block in blocks {
        for p in block.paragraphs() {
            alignment = alignment.or(p.properties.alignment);
            parts.push(paragraph_text(p, ctx).replace(['\n', '\u{c}'], " "));
        }
        if matches!(block, Block::Table(_)) {
            ctx.lose("nested table (flattened to text)");
        }
    }
    (parts.join(" "), alignment)
}

fn width(text: &str) -> usize {
    text.chars().count()
}

fn pad(text: &str, to: usize, alignment: Option<Alignment>) -> String {
    let gap = to.saturating_sub(width(text));
    match alignment {
        Some(Alignment::Right) => format!("{}{text}", " ".repeat(gap)),
        Some(Alignment::Center) => {
            let left = gap / 2;
            format!("{}{text}{}", " ".repeat(left), " ".repeat(gap - left))
        }
        _ => format!("{text}{}", " ".repeat(gap)),
    }
}

pub(super) fn table(table: &Table, ctx: &mut Ctx<'_>) -> String {
    let mut grid: Vec<Vec<(String, Option<Alignment>)>> = Vec::new();
    for slots in table.grid() {
        let mut row = Vec::with_capacity(slots.len());
        for slot in slots {
            row.push(match slot {
                Slot::Cell(cell) => {
                    if cell.properties.col_span > 1 || cell.properties.row_span > 1 {
                        ctx.lose("merged cell (text in its first position)");
                    }
                    cell_text(&cell.blocks, ctx)
                }
                Slot::Covered | Slot::Empty => (String::new(), None),
            });
        }
        grid.push(row);
    }
    let cols = grid.first().map_or(0, Vec::len);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            grid.iter()
                .map(|r| width(&r[c].0))
                .max()
                .unwrap_or(0)
                .max(1)
        })
        .collect();
    let line = |row: &[(String, Option<Alignment>)]| {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|((text, alignment), w)| pad(text, *w, *alignment))
            .collect();
        cells.join(" | ").trim_end().to_owned()
    };
    let header = table
        .rows
        .first()
        .is_some_and(|r| !r.cells.is_empty() && r.cells.iter().all(|c| c.header));
    let mut lines = Vec::new();
    for (i, row) in grid.iter().enumerate() {
        lines.push(line(row));
        if i == 0 && header {
            let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            lines.push(rule.join("-+-"));
        }
    }
    lines.join("\n")
}

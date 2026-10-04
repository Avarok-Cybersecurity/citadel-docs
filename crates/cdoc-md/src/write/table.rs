//! GFM pipe tables. Markdown tables have a header row and one block of inline text per cell,
//! so spans, nested blocks and per-cell formatting are flattened and reported.

use super::block::{paragraph, Chunk};
use super::Ctx;
use cdoc_model::{Alignment, Block, Slot, Table};

fn cell_text(blocks_in: &[Block], ctx: &mut Ctx<'_>) -> String {
    ctx.in_table = true;
    let mut chunks = Vec::new();
    for block in blocks_in {
        match block {
            Block::Paragraph(p) => chunks.extend(paragraph(p, ctx)),
            Block::Table(_) => {
                ctx.lose("nested table (flattened to text)");
                for p in block.paragraphs() {
                    chunks.extend(paragraph(p, ctx));
                }
            }
        }
    }
    ctx.in_table = false;
    chunks
        .into_iter()
        .map(|c| match c {
            Chunk::Block(t) | Chunk::Item(t) => t.replace('\n', "<br>"),
        })
        .collect::<Vec<_>>()
        .join("<br>")
}

fn first_alignment(cell: &[Block]) -> Option<Alignment> {
    cell.iter().find_map(|b| match b {
        Block::Paragraph(p) => p.properties.alignment,
        Block::Table(_) => None,
    })
}

pub(super) fn table(table: &Table, ctx: &mut Ctx<'_>) -> String {
    let cols = table.column_widths.len().max(1);
    let mut aligns: Vec<Option<Alignment>> = vec![None; cols];
    let mut grid: Vec<Vec<String>> = Vec::new();
    for (r, slots) in table.grid().into_iter().enumerate() {
        let mut cells = Vec::with_capacity(cols);
        for (col, slot) in slots.into_iter().enumerate() {
            let Slot::Cell(cell) = slot else {
                cells.push(String::new());
                continue;
            };
            if r == 1 {
                aligns[col] = first_alignment(&cell.blocks);
            }
            if cell.properties.col_span > 1 || cell.properties.row_span > 1 {
                ctx.lose("merged cell (flattened)");
            }
            cells.push(cell_text(&cell.blocks, ctx));
        }
        grid.push(cells);
    }
    if table
        .rows
        .first()
        .is_some_and(|r| !r.cells.iter().all(|c| c.header))
    {
        ctx.lose("table without a header row (first row written as the header)");
    }
    if table.rows.len() < 2 {
        aligns = table
            .rows
            .first()
            .map(|r| r.cells.iter().map(|c| first_alignment(&c.blocks)).collect())
            .unwrap_or_default();
        aligns.resize(cols, None);
    }
    let line = |cells: &[String]| format!("| {} |", cells.join(" | "));
    let rule: Vec<String> = aligns
        .iter()
        .map(|a| match a {
            Some(Alignment::Left) => ":---",
            Some(Alignment::Center) => ":---:",
            Some(Alignment::Right) => "---:",
            _ => "---",
        })
        .map(str::to_owned)
        .collect();
    let mut lines = Vec::new();
    for (i, cells) in grid.iter().enumerate() {
        lines.push(line(cells));
        if i == 0 {
            lines.push(line(&rule));
        }
    }
    lines.join("\n")
}

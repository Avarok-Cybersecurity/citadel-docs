//! Deleting a row or a column: spans crossing it shrink, and a spanning cell that starts in
//! the deleted line moves its content to the line that remains.

use super::table_grid::{caret_place, cell_atom};
use super::{done, Ctx, Outcome};
use crate::error::EditError;
use crate::position::{Position, Selection};
use crate::restore::insert_pieces;
use cdoc_crdt::slice;
use cdoc_model::{CellProperties, Placement};

/// Deletes the caret's row; a cell spanning down from it moves its content to the next row.
pub(crate) fn delete_row(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let t = ctx.caret_table()?;
    if t.rows.len() == 1 {
        return super::table::delete(ctx);
    }
    let places = t.model.placements();
    let r = caret_place(&t, &places)?.row;
    for p in places
        .iter()
        .filter(|p| p.row < r && r < p.row + p.row_span)
    {
        let (c, rs) = (p.cell.properties.col_span, p.cell.properties.row_span - 1);
        ctx.set_span(&t, p, c, rs)?;
    }
    let mut moves = Vec::new();
    for p in places.iter().filter(|p| p.row == r && p.row_span > 1) {
        let units = ctx.cell_units(&t, r, p.index)?;
        let content = slice(ctx.tx.doc, units.start + 1..units.end)?;
        let props = CellProperties {
            row_span: p.cell.properties.row_span - 1,
            ..p.cell.properties.clone()
        };
        let col = places
            .iter()
            .find(|q| q.row == r + 1 && q.col > p.col)
            .map_or(usize::MAX, |q| q.col);
        moves.push((
            ctx.slot(&t, &places, r + 1, col)?,
            cell_atom(p.cell, props),
            content,
        ));
    }
    for (at, cell, content) in moves.into_iter().rev() {
        ctx.tx.insert_atom(at, &cell)?;
        insert_pieces(&mut ctx.tx, at + 1, &content)?;
    }
    let target = t
        .rows
        .get(r + 1)
        .or_else(|| t.rows.get(r - 1))
        .map(|row| row.1[0]);
    let start = ctx.node_index(t.rows[r].0)?;
    let end = ctx.node_index(t.rows.get(r + 1).map_or(t.end, |row| row.0))?;
    ctx.tx.delete(start..end)?;
    let caret = target
        .and_then(|cell| ctx.outline.nodes.get(cell + 1))
        .map(|n| Position::after(n.id));
    Ok(done(caret.map_or(ctx.selection, Selection::caret)))
}

/// Deletes the caret's grid column; a cell spanning it narrows instead.
pub(crate) fn delete_column(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let t = ctx.caret_table()?;
    if t.model.column_widths.len() == 1 {
        return super::table::delete(ctx);
    }
    let places = t.model.placements();
    let p0 = caret_place(&t, &places)?;
    let c = p0.col;
    let hit = |p: &Placement<'_>| p.col <= c && c < p.col + p.col_span;
    for (r, row) in t.rows.iter().enumerate() {
        let gone = places
            .iter()
            .filter(|p| p.row == r && hit(p) && p.col_span == 1)
            .count();
        if gone == row.1.len() {
            return Err(EditError::NotApplicable(
                "deleting the column would empty a row",
            ));
        }
    }
    let mut deletes = Vec::new();
    for p in places.iter().filter(|p| hit(p)) {
        if p.col_span > 1 {
            let (cs, rs) = (p.cell.properties.col_span - 1, p.cell.properties.row_span);
            ctx.set_span(&t, p, cs, rs)?;
        } else {
            deletes.push(ctx.cell_units(&t, p.row, p.index)?);
        }
    }
    let mut widths = t.model.column_widths.clone();
    widths.remove(c);
    ctx.set_widths(&t, widths)?;
    deletes.sort_by_key(|r| r.start);
    for units in deletes.into_iter().rev() {
        ctx.tx.delete(units)?;
    }
    let row = &t.rows[p0.row].1;
    let keep = places
        .iter()
        .filter(|p| p.row == p0.row && !(hit(p) && p.col_span == 1))
        .map(|p| row[p.index])
        .next();
    let caret = keep
        .and_then(|cell| ctx.outline.nodes.get(cell + 1))
        .map(|n| Position::after(n.id));
    Ok(done(caret.map_or(ctx.selection, Selection::caret)))
}

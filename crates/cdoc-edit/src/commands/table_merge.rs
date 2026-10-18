//! Merging cells: the rectangle from the anchor's cell to the head's, grown until no cell
//! straddles its edge, becomes its top-left cell with the spans of the rectangle; the other
//! cells' content is appended to it (empty ones are dropped), and a row left with no cells is
//! removed, shrinking the spans that crossed it.

use super::table_grid::caret_place;
use super::{done, Ctx, Outcome};
use crate::error::EditError;
use crate::position::{Position, Selection};
use crate::restore::insert_pieces;
use cdoc_crdt::{slice, Atom, Piece};
use cdoc_model::Placement;

type Rect = (usize, usize, usize, usize);

fn grow(places: &[Placement<'_>], mut r: Rect) -> Rect {
    loop {
        let before = r;
        for p in places {
            let (pr1, pc1) = (p.row + p.row_span, p.col + p.col_span);
            if p.row < r.1 && pr1 > r.0 && p.col < r.3 && pc1 > r.2 {
                r = (r.0.min(p.row), r.1.max(pr1), r.2.min(p.col), r.3.max(pc1));
            }
        }
        if r == before {
            return r;
        }
    }
}

/// Merges the cells between the anchor and the head.
pub(crate) fn merge(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let doc = &*ctx.tx.doc;
    let (a, h) = (
        ctx.selection.anchor.index(doc)?,
        ctx.selection.head.index(doc)?,
    );
    let (ca, ch) = (ctx.cell_at(a)?, ctx.cell_at(h)?);
    let t = ctx.table_of(ca)?;
    let th = ctx.table_of(ch)?;
    if t.table != th.table {
        return Err(EditError::NotApplicable(
            "the cells are in different tables",
        ));
    }
    let places = t.model.placements();
    let pa = caret_place(&t, &places)?;
    let ph = caret_place(&th, &places)?;
    let r = grow(
        &places,
        (
            pa.row.min(ph.row),
            (pa.row + pa.row_span).max(ph.row + ph.row_span),
            pa.col.min(ph.col),
            (pa.col + pa.col_span).max(ph.col + ph.col_span),
        ),
    );
    let inside = |p: &Placement<'_>| p.row >= r.0 && p.row < r.1 && p.col >= r.2 && p.col < r.3;
    let Some(top) = places
        .iter()
        .find(|p| p.row == r.0 && p.col == r.2)
        .copied()
    else {
        return Err(EditError::NotApplicable(
            "the merged area has no top-left cell",
        ));
    };
    let others: Vec<Placement<'_>> = places
        .iter()
        .filter(|p| inside(p) && (p.row, p.index) != (top.row, top.index))
        .copied()
        .collect();
    if others.is_empty() {
        return Err(EditError::NotApplicable("nothing to merge"));
    }
    let emptied: Vec<usize> = (r.0 + 1..r.1)
        .filter(|&row| places.iter().filter(|p| p.row == row).all(&inside))
        .collect();
    let mut moved: Vec<Piece> = Vec::new();
    let mut deletes = Vec::new();
    for p in &others {
        let units = ctx.cell_units(&t, p.row, p.index)?;
        let content = slice(ctx.tx.doc, units.start + 1..units.end)?;
        if !matches!(content.as_slice(), [Piece::Block(Atom::Paragraph(_))]) {
            moved.extend(content);
        }
        if !emptied.contains(&p.row) {
            deletes.push(units);
        }
    }
    for &row in &emptied {
        let start = ctx.node_index(t.rows[row].0)?;
        let end = ctx.node_index(t.rows.get(row + 1).map_or(t.end, |next| next.0))?;
        deletes.push(start..end);
    }
    let removed = emptied.len();
    for p in places.iter().filter(|p| !inside(p)) {
        let crossed = emptied
            .iter()
            .filter(|&&row| p.row < row && row < p.row + p.row_span)
            .count();
        if crossed > 0 {
            let rs = u32::try_from(p.row_span - crossed).unwrap_or(1);
            ctx.set_span(&t, p, p.cell.properties.col_span, rs)?;
        }
    }
    let spans = (u32::try_from(r.3 - r.2), u32::try_from(r.1 - r.0 - removed));
    let (Ok(cs), Ok(rs)) = spans else {
        return Err(EditError::InvalidArgument("the merged area is too large"));
    };
    ctx.set_span(&t, &top, cs, rs)?;
    let end_of_top = ctx.cell_units(&t, top.row, top.index)?.end;
    deletes.sort_by_key(|d| d.start);
    for d in deletes.into_iter().rev() {
        ctx.tx.delete(d)?;
    }
    insert_pieces(&mut ctx.tx, end_of_top, &moved)?;
    let cell = t.rows[top.row].1[top.index];
    let caret = ctx
        .outline
        .nodes
        .get(cell + 1)
        .map(|n| Position::after(n.id));
    Ok(done(caret.map_or(ctx.selection, Selection::caret)))
}

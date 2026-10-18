//! Row and column edits that keep spans valid: a span crossing the edited line grows or
//! shrinks; a cell that would be cut keeps its content in the row that remains.

use super::table::{new_cell, TableAt};
use super::{done, Ctx, Outcome};
use crate::error::EditError;
use cdoc_crdt::Atom;
use cdoc_model::{Block, CellProperties, Placement, TableCell};

pub(super) fn cell_atom(cell: &TableCell, properties: CellProperties) -> Atom {
    Atom::Cell {
        properties,
        header: cell.header,
    }
}

fn first_paragraph(cell: &TableCell) -> Atom {
    match cell.blocks.first() {
        Some(Block::Paragraph(p)) => Atom::Paragraph(p.properties.clone()),
        _ => super::table::empty_paragraph(),
    }
}

impl Ctx<'_, '_> {
    pub(crate) fn caret_table(&self) -> Result<TableAt, EditError> {
        let gap = self.selection.head.index(self.tx.doc)?;
        self.table_of(self.cell_at(gap)?)
    }

    pub(crate) fn set_span(
        &mut self,
        t: &TableAt,
        p: &Placement<'_>,
        col: u32,
        row: u32,
    ) -> Result<(), EditError> {
        let at = self.node_index(t.rows[p.row].1[p.index])?;
        let properties = CellProperties {
            col_span: col,
            row_span: row,
            ..p.cell.properties.clone()
        };
        self.tx.set_block(at, &cell_atom(p.cell, properties))
    }

    /// Where a cell starting at grid column `col` goes in row `row`: before the first cell of
    /// the row placed at or after it, else at the row's end.
    pub(super) fn slot(
        &self,
        t: &TableAt,
        places: &[Placement<'_>],
        row: usize,
        col: usize,
    ) -> Result<usize, EditError> {
        match places.iter().find(|p| p.row == row && p.col >= col) {
            Some(p) => self.node_index(t.rows[row].1[p.index]),
            None => self.node_index(t.rows.get(row + 1).map_or(t.end, |r| r.0)),
        }
    }

    pub(super) fn set_widths(
        &mut self,
        t: &TableAt,
        widths: Vec<cdoc_model::Length>,
    ) -> Result<(), EditError> {
        let at = self.node_index(t.table)?;
        let atom = Atom::Table {
            properties: t.model.properties.clone(),
            column_widths: widths,
        };
        self.tx.set_block(at, &atom)
    }
}

pub(crate) fn caret_place<'t>(
    t: &'t TableAt,
    places: &[Placement<'t>],
) -> Result<Placement<'t>, EditError> {
    places
        .iter()
        .find(|p| (p.row, p.index) == t.caret)
        .copied()
        .ok_or(EditError::NotApplicable(
            "the caret cell is outside the table grid",
        ))
}

/// A row above or below the caret's.
pub(crate) fn insert_row(ctx: &mut Ctx<'_, '_>, below: bool) -> Result<Outcome, EditError> {
    let t = ctx.caret_table()?;
    let places = t.model.placements();
    let p0 = caret_place(&t, &places)?;
    let k = if below { p0.row + p0.row_span } else { p0.row };
    let reference = if below { k - 1 } else { k };
    let covers = |p: &Placement<'_>| p.row <= reference && reference < p.row + p.row_span;
    let crosses = |p: &Placement<'_>| p.row < k && k < p.row + p.row_span;
    let fresh: Vec<&Placement<'_>> = places.iter().filter(|p| covers(p) && !crosses(p)).collect();
    if fresh.is_empty() {
        return Err(EditError::NotApplicable(
            "every cell of the row spans across it",
        ));
    }
    for p in places.iter().filter(|p| crosses(p)) {
        let (c, r) = (p.cell.properties.col_span, p.cell.properties.row_span + 1);
        ctx.set_span(&t, p, c, r)?;
    }
    let mut at = ctx.node_index(t.rows.get(k).map_or(t.end, |r| r.0))?;
    let row = &t.model.rows[reference];
    let mut atoms = vec![Atom::Row {
        height: row.height,
        cant_split: row.cant_split,
    }];
    for p in fresh {
        atoms.push(cell_atom(
            p.cell,
            CellProperties {
                row_span: 1,
                ..p.cell.properties.clone()
            },
        ));
        atoms.push(first_paragraph(p.cell));
    }
    for atom in &atoms {
        ctx.tx.insert_atom(at, atom)?;
        at += 1;
    }
    Ok(done(ctx.selection))
}

/// A column left or right of the caret's.
pub(crate) fn insert_column(ctx: &mut Ctx<'_, '_>, right: bool) -> Result<Outcome, EditError> {
    let t = ctx.caret_table()?;
    let places = t.model.placements();
    let p0 = caret_place(&t, &places)?;
    let c = if right { p0.col + p0.col_span } else { p0.col };
    let crosses = |p: &Placement<'_>| p.col < c && c < p.col + p.col_span;
    let mut inserts = Vec::new();
    for (r, _) in t.rows.iter().enumerate() {
        let covered = places
            .iter()
            .any(|p| crosses(p) && p.row <= r && r < p.row + p.row_span);
        if !covered {
            inserts.push(ctx.slot(&t, &places, r, c)?);
        }
    }
    for p in places.iter().filter(|p| crosses(p)) {
        let (cs, rs) = (p.cell.properties.col_span + 1, p.cell.properties.row_span);
        ctx.set_span(&t, p, cs, rs)?;
    }
    let mut widths = t.model.column_widths.clone();
    let from = if right { c - 1 } else { c };
    let template = widths[from.min(widths.len() - 1)];
    widths.insert(c, template);
    ctx.set_widths(&t, widths)?;
    for at in inserts.into_iter().rev() {
        ctx.tx.insert_atom(at, &super::table::empty_paragraph())?;
        ctx.tx.insert_atom(at, &new_cell(1, 1))?;
    }
    Ok(done(ctx.selection))
}

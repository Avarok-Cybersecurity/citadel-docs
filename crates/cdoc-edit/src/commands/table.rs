//! Tables: finding the table, row and cell around the caret, inserting and deleting a whole
//! table. Grid edits are in `table_grid.rs`, merging in `table_merge.rs`.

use super::{delete, done, Ctx, Outcome};
use crate::error::EditError;
use crate::outline::Owner;
use crate::position::{Position, Selection};
use crate::text::paragraph;
use cdoc_crdt::{fold_blocks, slice, Atom, AtomKind};
use cdoc_model::{
    Block, Border, BorderStyle, BorderWidth, Borders, CellProperties, Length, ParagraphProperties,
    Table, TableProperties, TableWidth,
};

/// A table as the outline and the model see it.
pub(crate) struct TableAt {
    pub(crate) table: usize,
    pub(crate) end: usize,
    /// Each row's node and its cells' nodes.
    pub(crate) rows: Vec<(usize, Vec<usize>)>,
    pub(crate) model: Table,
    /// The caret cell: row index and index in the row.
    pub(crate) caret: (usize, usize),
}

impl Ctx<'_, '_> {
    /// The visible index of node `n`.
    pub(crate) fn node_index(&self, n: usize) -> Result<usize, EditError> {
        let id = self
            .outline
            .nodes
            .get(n)
            .ok_or(EditError::UnknownPosition)?
            .id;
        Ok(self
            .tx
            .doc
            .locate(id)
            .ok_or(EditError::UnknownPosition)?
            .index)
    }

    /// The cell node holding caret gap `gap`.
    pub(crate) fn cell_at(&self, gap: usize) -> Result<usize, EditError> {
        let (p, _) = paragraph(self.tx.doc, &self.outline, gap)?;
        self.outline.nodes[p]
            .parent
            .filter(|&c| self.outline.nodes[c].kind == AtomKind::Cell)
            .ok_or(EditError::NotApplicable("the caret is not in a table"))
    }

    /// The table whose cell holds `cell`.
    pub(crate) fn table_of(&self, cell: usize) -> Result<TableAt, EditError> {
        self.require_regular()?;
        let nodes = &self.outline.nodes;
        let row = nodes[cell].parent.ok_or(EditError::Irregular)?;
        let table = nodes[row].parent.ok_or(EditError::Irregular)?;
        let end = nodes[table].end.ok_or(EditError::Irregular)?;
        let mut rows: Vec<(usize, Vec<usize>)> = Vec::new();
        let mut caret = (0, 0);
        for n in table + 1..end {
            match nodes[n].kind {
                AtomKind::Row if nodes[n].parent == Some(table) => rows.push((n, Vec::new())),
                AtomKind::Cell
                    if nodes[n]
                        .parent
                        .is_some_and(|r| nodes[r].parent == Some(table)) =>
                {
                    let row = rows.len().checked_sub(1).ok_or(EditError::Irregular)?;
                    if n == cell {
                        caret = (row, rows[row].1.len());
                    }
                    rows[row].1.push(n);
                }
                _ => {}
            }
        }
        let range = self.node_index(table)?..self.node_index(end)? + 1;
        let Some(Block::Table(model)) = fold_blocks(slice(self.tx.doc, range)?).into_iter().next()
        else {
            return Err(EditError::Irregular);
        };
        Ok(TableAt {
            table,
            end,
            rows,
            model,
            caret,
        })
    }

    /// The units of cell node `cell` (its start atom to the next cell, row or table end).
    pub(crate) fn cell_units(
        &self,
        t: &TableAt,
        row: usize,
        index: usize,
    ) -> Result<std::ops::Range<usize>, EditError> {
        let cells = &t.rows[row].1;
        let next = cells
            .get(index + 1)
            .or_else(|| t.rows.get(row + 1).map(|r| &r.0))
            .copied()
            .unwrap_or(t.end);
        Ok(self.node_index(cells[index])?..self.node_index(next)?)
    }
}

/// Word's "Table Grid": half-point single lines.
fn grid() -> Borders {
    Borders::all(Border {
        style: BorderStyle::Single,
        width: BorderWidth(4),
        color: None,
        space: Length(0),
    })
}

/// A new empty cell with the grid borders.
pub(crate) fn new_cell(col_span: u32, row_span: u32) -> Atom {
    Atom::Cell {
        properties: CellProperties {
            col_span,
            row_span,
            borders: Some(grid()),
            ..CellProperties::single()
        },
        header: false,
    }
}

/// An empty paragraph for a new cell.
pub(crate) fn empty_paragraph() -> Atom {
    Atom::Paragraph(ParagraphProperties::none())
}

/// Inserts a `rows` × `cols` table at the caret, splitting its paragraph there.
pub(crate) fn insert(ctx: &mut Ctx<'_, '_>, rows: u32, cols: u32) -> Result<Outcome, EditError> {
    if rows == 0 || cols == 0 || rows > 1000 || cols > 63 {
        return Err(EditError::InvalidArgument(
            "a table needs 1 to 1000 rows and 1 to 63 columns",
        ));
    }
    ctx.require_regular()?;
    let gap = delete::selection(ctx)?;
    ctx.refresh();
    let (node, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    let section = match ctx.outline.nodes[node].owner {
        Owner::Top { section, .. } | Owner::Section(section) | Owner::Furniture(section) => section,
        Owner::Footnote => 0,
        Owner::Stray => return Err(EditError::Irregular),
    };
    let page = &ctx.model.sections[section].properties.page;
    let text = page.size.width.0 - page.margins.left.0 - page.margins.right.0;
    let width = (text / i32::try_from(cols).unwrap_or(1)).max(1);
    let mut at = if gap > para.start {
        let Some(Atom::Paragraph(p)) = cdoc_crdt::atom_at(ctx.tx.doc, para.start - 1)? else {
            return Err(EditError::Irregular);
        };
        ctx.tx.insert_atom(gap, &Atom::Paragraph(p))?;
        gap
    } else {
        para.start - 1
    };
    let start = at;
    let table = Atom::Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: Some(grid()),
            cell_padding: None,
        },
        column_widths: vec![Length(width); cols as usize],
    };
    let mut put = |ctx: &mut Ctx<'_, '_>, atom: &Atom| -> Result<(), EditError> {
        ctx.tx.insert_atom(at, atom)?;
        at += 1;
        Ok(())
    };
    put(ctx, &table)?;
    for _ in 0..rows {
        put(
            ctx,
            &Atom::Row {
                height: None,
                cant_split: false,
            },
        )?;
        for _ in 0..cols {
            put(ctx, &new_cell(1, 1))?;
            put(ctx, &empty_paragraph())?;
        }
    }
    put(ctx, &Atom::End)?;
    let first = ctx
        .tx
        .doc
        .id_at(start + 3)
        .ok_or(EditError::UnknownPosition)?;
    Ok(done(Selection::caret(Position::after(first))))
}

/// Deletes the caret's table; an empty paragraph takes its place if it was its container's
/// only block.
pub(crate) fn delete(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let gap = ctx.selection.head.index(ctx.tx.doc)?;
    let t = ctx.table_of(ctx.cell_at(gap)?)?;
    let nodes = &ctx.outline.nodes;
    let parent = nodes[t.table].parent;
    let next = nodes
        .get(t.end + 1)
        .filter(|n| n.parent == parent && n.kind == AtomKind::Paragraph)
        .map(|n| n.id);
    let (start, end) = (ctx.node_index(t.table)?, ctx.node_index(t.end)? + 1);
    let caret = if let Some(id) = next {
        Position::after(id)
    } else {
        ctx.tx.insert_atom(end, &empty_paragraph())?;
        Position::after(ctx.tx.doc.id_at(end).ok_or(EditError::UnknownPosition)?)
    };
    ctx.tx.delete(start..end)?;
    Ok(done(Selection::caret(caret)))
}

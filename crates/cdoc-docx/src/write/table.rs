//! Tables. The model stores merged cells as spans on the top-left cell; `.docx` stores
//! `gridSpan` on every cell and a `vMerge` continuation cell in each covered row. The model's
//! `Table::placements` does the unpacking; this puts each cell, and each continuation, in its
//! row.

use super::Writer;
use crate::error::DocxError;
use crate::props::{twips, write_cell_properties, write_row_properties, write_table_properties};
use crate::xml::Node;
use cdoc_model::{CellProperties, Placement, Table, TableCell, TableRow};

impl Writer<'_> {
    pub(super) fn table(&mut self, table: &Table) -> Result<Node, DocxError> {
        let mut grid = Node::new("w:tblGrid");
        for width in &table.column_widths {
            let w = twips(*width, &mut self.log);
            grid = grid.child(Node::new("w:gridCol").num("w:w", w));
        }
        let mut node = Node::new("w:tbl")
            .child(write_table_properties(&table.properties, &mut self.log))
            .child(grid);
        let placements = table.placements();
        for (r, row) in table.rows.iter().enumerate() {
            node = node.child(self.row(r, row, &placements)?);
        }
        let cut_short = |p: &Placement<'_>| {
            usize::try_from(p.cell.properties.row_span).is_ok_and(|asked| asked > p.row_span)
        };
        if placements.iter().any(cut_short) {
            self.log.lose("a merged cell extends past the last row");
        }
        Ok(node)
    }

    fn row(
        &mut self,
        r: usize,
        row: &TableRow,
        placements: &[Placement<'_>],
    ) -> Result<Node, DocxError> {
        let repeat_header = !row.cells.is_empty() && row.cells.iter().all(|c| c.header);
        let trpr = write_row_properties(row, repeat_header, &mut self.log);
        let mut tr = Node::new("w:tr");
        if !trpr.children.is_empty() {
            tr = tr.child(trpr);
        }
        // This row's cells, and the continuations of cells that began above, left to right.
        let mut pieces: Vec<&Placement<'_>> = placements
            .iter()
            .filter(|p| p.row <= r && r < p.row + p.row_span)
            .collect();
        pieces.sort_by_key(|p| p.col);
        let mut next_col = 0;
        for p in &pieces {
            if p.col != next_col {
                self.log.lose("a merged cell sits beyond a gap in its row");
            }
            next_col = p.col + p.col_span;
            tr = tr.child(if p.row == r {
                self.cell(p)?
            } else {
                self.continuation(p)
            });
        }
        let placed = pieces.iter().filter(|p| p.row == r).count();
        for cell in row.cells.iter().skip(placed) {
            self.log
                .lose("a row has more cells than the table grid; written after the grid");
            tr = tr.child(self.loose_cell(cell)?);
        }
        Ok(tr)
    }

    fn cell(&mut self, p: &Placement<'_>) -> Result<Node, DocxError> {
        let properties = CellProperties {
            col_span: u32::try_from(p.col_span).unwrap_or(1),
            ..p.cell.properties.clone()
        };
        let vmerge = (p.row_span > 1).then_some(true);
        let tcpr = write_cell_properties(&properties, p.cell.header, vmerge, &mut self.log);
        self.tc(tcpr, p.cell)
    }

    fn loose_cell(&mut self, cell: &TableCell) -> Result<Node, DocxError> {
        let tcpr = write_cell_properties(&cell.properties, cell.header, None, &mut self.log);
        self.tc(tcpr, cell)
    }

    fn tc(&mut self, tcpr: Node, cell: &TableCell) -> Result<Node, DocxError> {
        let mut tc = Node::new("w:tc");
        if !tcpr.children.is_empty() {
            tc = tc.child(tcpr);
        }
        Ok(tc.children(self.blocks(&cell.blocks, None)?))
    }

    fn continuation(&mut self, p: &Placement<'_>) -> Node {
        let properties = CellProperties {
            col_span: u32::try_from(p.col_span).unwrap_or(1),
            ..CellProperties::single()
        };
        let tcpr = write_cell_properties(&properties, false, Some(false), &mut self.log);
        Node::new("w:tc").child(tcpr).child(Node::new("w:p"))
    }
}

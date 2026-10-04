//! Tables: grid, rows, and cells with vertical merges folded back into row spans.

use super::block::bare_paragraph;
use super::Reader;
use crate::error::DocxError;
use crate::props::{length, read_cell_properties, read_row_flags, read_table_properties};
use crate::xml::Node;
use cdoc_model::{Length, Table, TableCell, TableProperties, TableRow, TableWidth};

/// Where the cell that began a vertical merge lives: row index, cell index.
type Origin = Option<(usize, usize)>;

impl Reader<'_> {
    pub(super) fn table(&mut self, node: &Node) -> Result<Table, DocxError> {
        let properties = match node.find("w:tblPr") {
            Some(pr) => read_table_properties(pr, &mut self.log),
            None => TableProperties {
                style: None,
                width: TableWidth::Auto,
                alignment: None,
                borders: None,
                cell_padding: None,
            },
        };
        let mut column_widths: Vec<Length> = Vec::new();
        if let Some(grid) = node.find("w:tblGrid") {
            for col in grid.elems() {
                if let Some(w) = length(col, "w:w", &mut self.log) {
                    column_widths.push(w);
                }
            }
        }
        let mut rows: Vec<TableRow> = Vec::new();
        let mut origins: Vec<Origin> = vec![None; column_widths.len()];
        for child in node.elems() {
            match child.name.as_str() {
                "w:tr" => {
                    let row = self.row(child, &mut rows, &mut origins)?;
                    rows.push(row);
                }
                "w:tblPr" | "w:tblGrid" => {}
                other => self.log.unknown(&format!("w:tbl/{other}")),
            }
        }
        if column_widths.is_empty() {
            self.log
                .lose("a table without a grid; column widths of 1 inch assumed");
            column_widths = vec![Length::from_inches(1.0); origins.len().max(1)];
        }
        Ok(Table {
            properties,
            column_widths,
            rows,
        })
    }

    /// Read one row. `rows` holds the rows before it, so a continuation cell can extend the
    /// merge that began above.
    fn row(
        &mut self,
        node: &Node,
        rows: &mut [TableRow],
        origins: &mut Vec<Origin>,
    ) -> Result<TableRow, DocxError> {
        let (height, cant_split, header_row) = node
            .find("w:trPr")
            .map_or((None, false, false), |pr| read_row_flags(pr, &mut self.log));
        let row_index = rows.len();
        let mut cells: Vec<TableCell> = Vec::new();
        let mut col = 0usize;
        for tc in node.elems() {
            match tc.name.as_str() {
                "w:tc" => {}
                "w:trPr" => continue,
                other => {
                    self.log.unknown(&format!("w:tr/{other}"));
                    continue;
                }
            }
            let (properties, header, vmerge) = match tc.find("w:tcPr") {
                Some(pr) => read_cell_properties(pr, &mut self.log),
                None => (cdoc_model::CellProperties::single(), false, None),
            };
            let span = usize::try_from(properties.col_span).unwrap_or(1).max(1);
            if origins.len() < col + span {
                origins.resize(col + span, None);
            }
            let blocks = self.container_blocks(tc)?;
            if vmerge == Some(false) {
                if let Some((r, c)) = origins[col] {
                    let target = if r == row_index {
                        cells.get_mut(c)
                    } else {
                        rows.get_mut(r).and_then(|row| row.cells.get_mut(c))
                    };
                    if let Some(origin) = target {
                        origin.properties.row_span += 1;
                        if blocks.len() != 1 || blocks[0] != bare_paragraph() {
                            self.log
                                .lose("content of a vertically merged continuation cell");
                        }
                        col += span;
                        continue;
                    }
                }
                self.log
                    .lose("a vertical merge continuation with nothing above it");
            }
            let origin = (vmerge == Some(true)).then_some((row_index, cells.len()));
            for slot in &mut origins[col..col + span] {
                *slot = origin;
            }
            cells.push(TableCell {
                properties,
                header: header || header_row,
                blocks,
            });
            col += span;
        }
        Ok(TableRow {
            height,
            cant_split,
            cells,
        })
    }
}

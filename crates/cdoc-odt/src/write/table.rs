//! Tables: one automatic style per table, column, row and cell; leading all-header rows in
//! `table:table-header-rows`; spans as `table:number-columns-spanned` /
//! `number-rows-spanned` with `table:covered-table-cell` under them; a ragged row filled with
//! padding cells the sidecar names.

use super::para::SectionStart;
use super::registry::Auto;
use super::Writer;
use crate::props::{
    read_cell, read_column_width, read_row, read_table, write_cell, write_column, write_row,
    write_table,
};
use crate::residual::Residual;
use cdoc_model::{Length, Slot, Table, TableCell, TableProperties, TableRow};
use cdoc_pkg::xml::Node;
use cdoc_pkg::{Log, PackageError};

fn scratch() -> Log {
    Log::new("(readback)")
}

impl Writer<'_> {
    fn table_style(&mut self, t: &Table, start: Option<&SectionStart>) -> String {
        let mut props = write_table(&t.properties);
        if let Some(n) = start.and_then(|s| s.page_number) {
            props = props.num("style:page-number", n);
        }
        let back_props = read_table(&props, &["style:page-number"], &mut scratch());
        let back = Table {
            properties: TableProperties {
                style: t.properties.style.clone(),
                ..back_props
            },
            column_widths: t
                .column_widths
                .iter()
                .map(|w| read_column_width(&write_column(*w), &mut scratch()).unwrap_or(Length(0)))
                .collect(),
            rows: Vec::new(),
        };
        let mut auto = Auto::new("table", vec![props]);
        auto.parent = t.properties.style.as_ref().map(|s| self.names.of(s));
        auto.residual = Residual::between(t, &back, &["rows"]);
        if let Some(s) = start {
            auto.attrs
                .push(("style:master-page-name".to_owned(), s.master.clone()));
        }
        self.autos().name(auto)
    }

    fn row_style(&mut self, row: &TableRow) -> String {
        let props = write_row(row);
        let (height, cant_split) = read_row(&props, &mut scratch());
        let back = TableRow {
            height,
            cant_split,
            cells: Vec::new(),
        };
        let mut auto = Auto::new("table-row", vec![props]);
        auto.residual = Residual::between(row, &back, &["cells"]);
        self.autos().name(auto)
    }

    fn cell_style(&mut self, cell: &TableCell, spans: (u32, u32), header: bool) -> String {
        let props = write_cell(&cell.properties);
        let mut back_props = read_cell(&props, &mut scratch());
        (back_props.col_span, back_props.row_span) = spans;
        let back = TableCell {
            properties: back_props,
            header,
            blocks: Vec::new(),
        };
        let mut auto = Auto::new("table-cell", vec![props]);
        auto.residual = Residual::between(cell, &back, &["blocks"]);
        self.autos().name(auto)
    }

    fn cell(
        &mut self,
        cell: &TableCell,
        col_span: usize,
        row_span: usize,
        header: bool,
    ) -> Result<Node, PackageError> {
        let spans = (
            u32::try_from(col_span).unwrap_or(u32::MAX),
            u32::try_from(row_span).unwrap_or(u32::MAX),
        );
        let style = self.cell_style(cell, spans, header);
        let mut node = Node::new("table:table-cell").attr("table:style-name", &style);
        if col_span > 1 {
            node = node.num("table:number-columns-spanned", col_span);
        }
        if row_span > 1 {
            node = node.num("table:number-rows-spanned", row_span);
        }
        let blocks = self.container(&cell.blocks, None)?;
        Ok(node.attr("office:value-type", "string").children(blocks))
    }

    fn pad(&mut self) -> Node {
        let name = self.autos().unique(Auto::new("table-cell", Vec::new()));
        self.sidecar.pads.insert(name.clone());
        Node::new("table:table-cell").attr("table:style-name", &name)
    }

    /// A table; `start` when it is the first block of a section.
    pub(super) fn table(
        &mut self,
        t: &Table,
        start: Option<&SectionStart>,
    ) -> Result<Node, PackageError> {
        self.tables += 1;
        let style = self.table_style(t, start);
        let mut node = Node::new("table:table")
            .attr("table:name", &format!("Table{}", self.tables))
            .attr("table:style-name", &style);
        for width in &t.column_widths {
            let name = self
                .autos()
                .name(Auto::new("table-column", vec![write_column(*width)]));
            node = node.child(Node::new("table:table-column").attr("table:style-name", &name));
        }
        let placed = t.placements();
        let placed_count = placed.len();
        let total: usize = t.rows.iter().map(|r| r.cells.len()).sum();
        if placed_count < total {
            self.log.lose("a cell beyond the table's grid dropped");
        }
        let header_rows = t
            .rows
            .iter()
            .take_while(|r| r.cells.iter().all(|c| c.header))
            .count();
        let grid = t.grid();
        let mut header = Node::new("table:table-header-rows");
        for (r, row) in t.rows.iter().enumerate() {
            let in_header = r < header_rows;
            let mut row_node =
                Node::new("table:table-row").attr("table:style-name", &self.row_style(row));
            for (c, slot) in grid[r].iter().enumerate() {
                let placement = placed.iter().find(|p| p.row == r && p.col == c);
                let cell = match (slot, placement) {
                    (Slot::Cell(_), Some(p)) => {
                        self.cell(p.cell, p.col_span, p.row_span, in_header)?
                    }
                    (Slot::Covered, _) => Node::new("table:covered-table-cell"),
                    (Slot::Cell(_) | Slot::Empty, _) => self.pad(),
                };
                row_node = row_node.child(cell);
            }
            if in_header {
                header = header.child(row_node);
            } else {
                if r == header_rows && header_rows > 0 {
                    node = node.child(std::mem::replace(
                        &mut header,
                        Node::new("table:table-header-rows"),
                    ));
                }
                node = node.child(row_node);
            }
        }
        if header_rows == t.rows.len() && header_rows > 0 {
            node = node.child(header);
        }
        Ok(node)
    }
}

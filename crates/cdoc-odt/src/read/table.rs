//! Tables: the table, column, row and cell styles through the shared property readers and
//! their residuals; header rows; spans, with covered cells skipped and the writer's padding
//! cells dropped; repeated columns, rows and cells expanded up to a bound.

use super::body::Start;
use super::Reader;
use crate::props::{
    read_cell, read_column_width, read_row, read_table, CELL_PROPERTIES, COLUMN_PROPERTIES,
    ROW_PROPERTIES, TABLE_PROPERTIES,
};
use cdoc_model::{
    Block, Document, Length, Paragraph, ParagraphProperties, Table, TableCell, TableRow, TableWidth,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

/// How many times a repeated column, row or cell is expanded at most.
const MAX_REPEAT: usize = 1024;

/// A paragraph with nothing in it, for a container ODF left empty.
pub fn bare() -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties::none(),
        inlines: Vec::new(),
    })
}

impl<'a> Reader<'a> {
    fn props_of(
        &self,
        family: &str,
        name: Option<&str>,
        element: &str,
    ) -> (Option<&'a Node>, Option<&'a Node>) {
        let style = name
            .and_then(|n| self.index.style(self.part, family, n))
            .map(|(s, _)| s);
        (style, style.and_then(|s| s.find(element)))
    }

    fn repeat(&mut self, node: &Node, key: &str) -> usize {
        let n = node
            .get(key)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);
        if n > MAX_REPEAT {
            self.log
                .lose(&format!("a repetition beyond {MAX_REPEAT} cut short"));
        }
        n.min(MAX_REPEAT)
    }

    fn columns(&mut self, node: &Node, out: &mut Vec<Option<Length>>) {
        for c in node.elems() {
            match c.name.as_str() {
                "table:table-column" => {
                    let (_, props) =
                        self.props_of("table-column", c.get("table:style-name"), COLUMN_PROPERTIES);
                    let empty = Node::new(COLUMN_PROPERTIES);
                    let width = read_column_width(props.unwrap_or(&empty), &mut self.log);
                    let times = self.repeat(c, "table:number-columns-repeated");
                    out.extend(std::iter::repeat_n(width, times));
                }
                "table:table-columns"
                | "table:table-header-columns"
                | "table:table-column-group" => self.columns(c, out),
                _ => {}
            }
        }
    }

    fn cell(&mut self, c: &Node, header: bool) -> Result<Option<TableCell>, PackageError> {
        let name = c.get("table:style-name");
        if name.is_some_and(|n| self.sidecar.pads.contains(n)) {
            return Ok(None);
        }
        let (_, props) = self.props_of("table-cell", name, CELL_PROPERTIES);
        let empty = Node::new(CELL_PROPERTIES);
        let mut properties = read_cell(props.unwrap_or(&empty), &mut self.log);
        let span = |key: &str| {
            c.get(key)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1)
        };
        properties.col_span = span("table:number-columns-spanned");
        properties.row_span = span("table:number-rows-spanned");
        let native = TableCell {
            properties,
            header,
            blocks: Vec::new(),
        };
        let mut cell = match name {
            Some(n) => self.fixed("table-cell", n, native),
            None => native,
        };
        cell.blocks = self.container(c.elems())?.blocks;
        if cell.blocks.is_empty() {
            cell.blocks.push(bare());
        }
        Ok(Some(cell))
    }

    fn row(&mut self, r: &Node, header: bool) -> Result<TableRow, PackageError> {
        let name = r.get("table:style-name");
        let (_, props) = self.props_of("table-row", name, ROW_PROPERTIES);
        let empty = Node::new(ROW_PROPERTIES);
        let (height, cant_split) = read_row(props.unwrap_or(&empty), &mut self.log);
        let native = TableRow {
            height,
            cant_split,
            cells: Vec::new(),
        };
        let mut row = match name {
            Some(n) => self.fixed("table-row", n, native),
            None => native,
        };
        for c in r.elems() {
            match c.name.as_str() {
                "table:table-cell" => {
                    let times = self.repeat(c, "table:number-columns-repeated");
                    if let Some(cell) = self.cell(c, header)? {
                        row.cells.extend(std::iter::repeat_n(cell, times));
                    }
                }
                "table:covered-table-cell" => {
                    if c.elems().next().is_some() {
                        self.log.lose("content of a cell covered by a span");
                    }
                }
                other => self.log.lose(&format!("{other} in a table row")),
            }
        }
        Ok(row)
    }

    fn rows(
        &mut self,
        node: &Node,
        header: bool,
        out: &mut Vec<TableRow>,
    ) -> Result<(), PackageError> {
        for r in node.elems() {
            match r.name.as_str() {
                "table:table-row" => {
                    let times = self.repeat(r, "table:number-rows-repeated");
                    let row = self.row(r, header)?;
                    out.extend(std::iter::repeat_n(row, times));
                }
                "table:table-header-rows" => self.rows(r, true, out)?,
                "table:table-rows" | "table:table-row-group" => self.rows(r, header, out)?,
                "table:table-column"
                | "table:table-columns"
                | "table:table-header-columns"
                | "table:table-column-group" => {}
                "text:soft-page-break" => self.log.lose("a soft page break (ignored)"),
                other => self.log.lose(&format!("{other} in a table")),
            }
        }
        Ok(())
    }

    fn widths(
        &mut self,
        widths: Vec<Option<Length>>,
        table: TableWidth,
        rows: &[TableRow],
    ) -> Vec<Length> {
        let declared = widths.len();
        let spanned = rows
            .iter()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| c.properties.col_span as usize)
                    .sum::<usize>()
            })
            .max();
        let count = declared.max(spanned.unwrap_or(0)).max(1);
        if widths.len() == count && widths.iter().all(|w| w.is_some_and(|w| w.0 > 0)) {
            return widths.into_iter().flatten().collect();
        }
        self.log
            .lose("a table column without a width; the table's width shared equally");
        let page = Document::blank().sections[0].properties.page.clone();
        let text = page.size.width.0 - page.margins.left.0 - page.margins.right.0;
        let total = match table {
            TableWidth::Fixed { length } if length.0 > 0 => length.0,
            _ => text,
        };
        let share = Length((total / i32::try_from(count).unwrap_or(i32::MAX)).max(1));
        (0..count)
            .map(|i| {
                widths
                    .get(i)
                    .copied()
                    .flatten()
                    .filter(|w| w.0 > 0)
                    .unwrap_or(share)
            })
            .collect()
    }

    /// A table, and the master page it starts.
    pub(super) fn table(&mut self, node: &Node) -> Result<(Table, Option<Start>), PackageError> {
        let name = node.get("table:style-name");
        let (style, props) = self.props_of("table", name, TABLE_PROPERTIES);
        let empty = Node::new(TABLE_PROPERTIES);
        let props = props.unwrap_or(&empty);
        let mut properties = read_table(props, &["style:page-number"], &mut self.log);
        properties.style = style
            .and_then(|s| s.get("style:parent-style-name"))
            .map(|n| self.style_id("table", n));
        let mut widths = Vec::new();
        self.columns(node, &mut widths);
        let native_widths: Vec<Length> = widths.iter().map(|w| w.unwrap_or(Length(0))).collect();
        let native = Table {
            properties,
            column_widths: native_widths,
            rows: Vec::new(),
        };
        let mut table = match name {
            Some(n) => self.fixed("table", n, native),
            None => native,
        };
        let mut rows = Vec::new();
        self.rows(node, false, &mut rows)?;
        let fixed: Vec<Option<Length>> = table.column_widths.iter().map(|w| Some(*w)).collect();
        table.column_widths = self.widths(fixed, table.properties.width, &rows);
        table.rows = rows;
        let start = style
            .and_then(|s| s.get("style:master-page-name"))
            .filter(|m| !m.is_empty())
            .map(|master| Start {
                master: master.to_owned(),
                page_number: props.get("style:page-number").and_then(|n| n.parse().ok()),
            });
        Ok((table, start))
    }
}

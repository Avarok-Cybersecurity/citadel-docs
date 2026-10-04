//! GFM tables.

use super::{Pending, Reader};
use cdoc_model::{
    Alignment, Block, Border, BorderStyle, BorderWidth, Borders, CellProperties, Length, Paragraph,
    Table, TableCell, TableProperties, TableRow, TableWidth,
};
use pulldown_cmark::Alignment as MdAlignment;

/// A table being filled.
pub(super) struct TableBuild {
    aligns: Vec<MdAlignment>,
    rows: Vec<TableRow>,
    current: Vec<TableCell>,
    header: bool,
}

impl TableBuild {
    pub(super) const fn new(aligns: Vec<MdAlignment>) -> Self {
        Self {
            aligns,
            rows: Vec::new(),
            current: Vec::new(),
            header: false,
        }
    }
}

fn alignment(a: MdAlignment) -> Option<Alignment> {
    match a {
        MdAlignment::None => None,
        MdAlignment::Left => Some(Alignment::Left),
        MdAlignment::Center => Some(Alignment::Center),
        MdAlignment::Right => Some(Alignment::Right),
    }
}

impl Reader<'_> {
    pub(super) fn start_row(&mut self, header: bool) {
        self.flush();
        if let Some(t) = &mut self.table {
            t.current.clear();
            t.header = header;
        }
    }

    pub(super) fn end_row(&mut self) {
        self.flush();
        if let Some(t) = &mut self.table {
            let cells = std::mem::take(&mut t.current);
            t.rows.push(TableRow {
                height: None,
                cant_split: false,
                cells,
            });
        }
    }

    pub(super) fn begin_cell(&mut self) {
        let col = self.table.as_ref().map_or(0, |t| t.current.len());
        let align = self
            .table
            .as_ref()
            .and_then(|t| t.aligns.get(col))
            .copied()
            .and_then(alignment);
        self.begin(Pending::Cell(align));
    }

    pub(super) fn cell_paragraph(&mut self, paragraph: Paragraph) {
        if let Some(t) = &mut self.table {
            t.current.push(TableCell {
                properties: CellProperties::single(),
                header: t.header,
                blocks: vec![Block::Paragraph(paragraph)],
            });
        }
    }

    pub(super) fn end_table(&mut self) {
        let Some(mut build) = self.table.take() else {
            return;
        };
        let cols = build.aligns.len().max(1);
        for row in &mut build.rows {
            let header = row.cells.first().is_some_and(|c| c.header);
            while row.cells.len() < cols {
                row.cells.push(TableCell {
                    properties: CellProperties::single(),
                    header,
                    blocks: vec![Block::Paragraph(Paragraph {
                        properties: cdoc_model::ParagraphProperties::none(),
                        inlines: Vec::new(),
                    })],
                });
            }
        }
        let width = self.opts.text_width.0;
        let count = i32::try_from(cols).unwrap_or(i32::MAX);
        let each = width / count;
        let mut column_widths = vec![Length(each); cols];
        if let Some(last) = column_widths.last_mut() {
            *last = Length(width - each * (count - 1));
        }
        let edge = Border {
            style: BorderStyle::Single,
            width: BorderWidth(4),
            color: None,
            space: Length(0),
        };
        self.out.push(Block::Table(Table {
            properties: TableProperties {
                style: None,
                width: TableWidth::Auto,
                alignment: None,
                borders: Some(Borders::all(edge)),
                cell_padding: None,
            },
            column_widths,
            rows: build.rows,
        }));
    }
}

//! Folding the flat sequence back into the model tree.
//!
//! A well-formed sequence (the grammar on [`Atom`]) folds into exactly the tree it came from.
//! Concurrent edits can leave a sequence that is not well formed (one user deletes a table
//! while another types in it); folding is still total and deterministic, so every replica
//! shows the same thing:
//!
//! - inlines with no open paragraph open one with no direct formatting;
//! - a block in a table with no row or cell opens a plain row or cell for it;
//! - a row or cell outside any table, and an `End` with nothing to close, are ignored;
//! - a section, header, footer or footnote start closes everything open;
//! - a header or footer before the first section is ignored (its blocks join the body), and
//!   one for a slot already filled appends to it;
//! - blocks before the first section belong to the first section.

use super::{Atom, BridgeError, Furniture};
use cdoc_model::{
    Block, CellProperties, Footnote, HeaderFooter, HeaderFooterKind, Inline, InlineContent,
    Paragraph, ParagraphProperties, Section, Table, TableCell, TableRow,
};

#[derive(Debug, Clone, Copy)]
enum Target {
    Body,
    Furniture(Furniture, HeaderFooterKind),
    Footnote,
}

pub(crate) struct Builder {
    sections: Vec<Section>,
    footnotes: Vec<Footnote>,
    orphans: Vec<Block>,
    target: Target,
    tables: Vec<Table>,
    para: Option<Paragraph>,
}

const fn plain_row() -> TableRow {
    TableRow {
        height: None,
        cant_split: false,
        cells: Vec::new(),
    }
}

fn open_cell(table: &mut Table) -> &mut TableCell {
    if table.rows.is_empty() {
        table.rows.push(plain_row());
    }
    let row = table.rows.last_mut().expect("a row exists");
    if row.cells.is_empty() {
        row.cells.push(TableCell {
            properties: CellProperties::single(),
            header: false,
            blocks: Vec::new(),
        });
    }
    row.cells.last_mut().expect("a cell exists")
}

impl Builder {
    pub(crate) const fn new() -> Self {
        Self {
            sections: Vec::new(),
            footnotes: Vec::new(),
            orphans: Vec::new(),
            target: Target::Body,
            tables: Vec::new(),
            para: None,
        }
    }

    fn container(&mut self) -> &mut Vec<Block> {
        match self.target {
            Target::Body => match self.sections.last_mut() {
                Some(s) => &mut s.blocks,
                None => &mut self.orphans,
            },
            Target::Furniture(furniture, kind) => {
                let s = self
                    .sections
                    .last_mut()
                    .expect("furniture belongs to a section");
                let set = match furniture {
                    Furniture::Header => &mut s.properties.headers,
                    Furniture::Footer => &mut s.properties.footers,
                };
                let hf = set
                    .slot(kind)
                    .get_or_insert_with(|| HeaderFooter { blocks: Vec::new() });
                &mut hf.blocks
            }
            Target::Footnote => {
                &mut self
                    .footnotes
                    .last_mut()
                    .expect("a footnote is open")
                    .blocks
            }
        }
    }

    fn push_block(&mut self, block: Block) {
        match self.tables.last_mut() {
            Some(table) => open_cell(table).blocks.push(block),
            None => self.container().push(block),
        }
    }

    fn close_paragraph(&mut self) {
        if let Some(p) = self.para.take() {
            self.push_block(Block::Paragraph(p));
        }
    }

    fn close_all(&mut self) {
        self.close_paragraph();
        while let Some(t) = self.tables.pop() {
            self.push_block(Block::Table(t));
        }
    }

    pub(crate) fn inline(&mut self, inline: Inline) {
        let para = self.para.get_or_insert_with(|| Paragraph {
            properties: ParagraphProperties::none(),
            inlines: Vec::new(),
        });
        if let (Some(last), InlineContent::Text(run)) = (para.inlines.last_mut(), &inline.content) {
            if let InlineContent::Text(prev) = &mut last.content {
                if prev.properties == run.properties && last.marks == inline.marks {
                    prev.text.push_str(&run.text);
                    return;
                }
            }
        }
        para.inlines.push(inline);
    }

    pub(crate) fn atom(&mut self, atom: Atom) {
        match atom {
            Atom::Inline(content) => self.inline(Inline::plain(content)),
            Atom::Paragraph(properties) => {
                self.close_paragraph();
                self.para = Some(Paragraph {
                    properties,
                    inlines: Vec::new(),
                });
            }
            Atom::Table {
                properties,
                column_widths,
            } => {
                self.close_paragraph();
                self.tables.push(Table {
                    properties,
                    column_widths,
                    rows: Vec::new(),
                });
            }
            Atom::Row { height, cant_split } => {
                self.close_paragraph();
                if let Some(t) = self.tables.last_mut() {
                    t.rows.push(TableRow {
                        height,
                        cant_split,
                        cells: Vec::new(),
                    });
                }
            }
            Atom::Cell { properties, header } => {
                self.close_paragraph();
                if let Some(t) = self.tables.last_mut() {
                    if t.rows.is_empty() {
                        t.rows.push(plain_row());
                    }
                    let row = t.rows.last_mut().expect("a row exists");
                    row.cells.push(TableCell {
                        properties,
                        header,
                        blocks: Vec::new(),
                    });
                }
            }
            Atom::End => {
                self.close_paragraph();
                match self.tables.pop() {
                    Some(t) => self.push_block(Block::Table(t)),
                    None => self.target = Target::Body,
                }
            }
            Atom::Section(properties) => {
                self.close_all();
                self.sections.push(Section {
                    properties,
                    blocks: Vec::new(),
                });
                self.target = Target::Body;
            }
            Atom::HeaderFooter { furniture, kind } => {
                self.close_all();
                self.target = if self.sections.is_empty() {
                    Target::Body
                } else {
                    Target::Furniture(furniture, kind)
                };
                if !self.sections.is_empty() {
                    self.container();
                }
            }
            Atom::Footnote(id) => {
                self.close_all();
                self.footnotes.push(Footnote {
                    id,
                    blocks: Vec::new(),
                });
                self.target = Target::Footnote;
            }
        }
    }

    pub(crate) fn finish(mut self) -> Result<(Vec<Section>, Vec<Footnote>), BridgeError> {
        self.close_all();
        let first = self.sections.first_mut().ok_or(BridgeError::NoSection)?;
        first.blocks.splice(0..0, self.orphans);
        Ok((self.sections, self.footnotes))
    }
}

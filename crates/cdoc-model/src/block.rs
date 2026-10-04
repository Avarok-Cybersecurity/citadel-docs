//! Block content: what a section, cell, header, footer or footnote holds.

use crate::paragraph::Paragraph;
use crate::table::Table;
use serde::{Deserialize, Serialize};

/// A block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    /// A paragraph.
    Paragraph(Paragraph),
    /// A table.
    Table(Table),
}

impl Block {
    /// Every paragraph in this block, descending into table cells.
    pub fn paragraphs(&self) -> Box<dyn Iterator<Item = &Paragraph> + '_> {
        match self {
            Self::Paragraph(p) => Box::new(std::iter::once(p)),
            Self::Table(t) => Box::new(
                t.rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .flat_map(|c| c.blocks.iter())
                    .flat_map(Block::paragraphs),
            ),
        }
    }

    /// Every table in this block, including nested ones, outermost first.
    pub fn tables(&self) -> Box<dyn Iterator<Item = &Table> + '_> {
        match self {
            Self::Paragraph(_) => Box::new(std::iter::empty()),
            Self::Table(t) => Box::new(
                std::iter::once(t).chain(
                    t.rows
                        .iter()
                        .flat_map(|r| r.cells.iter())
                        .flat_map(|c| c.blocks.iter())
                        .flat_map(Block::tables),
                ),
            ),
        }
    }
}

//! Positions as other coordinates: `(block, offset)` (blocks numbered as the CRDT's change
//! reports number them, so a change report and a position agree) and a path through the
//! model's body (section, block, then `(row, cell, block)` steps into tables, then the unit
//! offset in the paragraph).

use crate::editor::Editor;
use crate::error::EditError;
use crate::outline::Owner;
use crate::position::Position;
use crate::text::paragraph;
use cdoc_crdt::AtomKind;

/// A gap as a block and an offset into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockOffset {
    /// The block: the `n`-th structural atom of the replica (`cdoc_crdt::Changes` numbering).
    pub block: usize,
    /// Units after the block's start atom.
    pub offset: usize,
}

/// A gap as a path through the model's body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPath {
    /// The section.
    pub section: usize,
    /// The top-level block in the section.
    pub block: usize,
    /// Into tables: row, cell in the row, block in the cell.
    pub cells: Vec<(usize, usize, usize)>,
    /// Units into the paragraph.
    pub offset: usize,
}

impl Editor<'_> {
    /// `pos` as a block and offset.
    pub fn block_offset(&self, pos: Position) -> Result<BlockOffset, EditError> {
        let gap = pos.index(&self.doc)?;
        let block = self
            .doc
            .blocks_before(gap)
            .checked_sub(1)
            .ok_or(EditError::UnknownPosition)?;
        let start = self
            .doc
            .locate(self.outline.nodes[block].id)
            .ok_or(EditError::UnknownPosition)?
            .index;
        Ok(BlockOffset {
            block,
            offset: gap - start - 1,
        })
    }

    /// The position `at` names.
    pub fn position_of_block(&self, at: BlockOffset) -> Result<Position, EditError> {
        let node = self
            .outline
            .nodes
            .get(at.block)
            .ok_or(EditError::UnknownPosition)?;
        let start = self
            .doc
            .locate(node.id)
            .ok_or(EditError::UnknownPosition)?
            .index;
        Position::at(&self.doc, start + 1 + at.offset).ok_or(EditError::UnknownPosition)
    }

    /// `pos` as a path through the body; `None` outside it (headers, footers, footnotes).
    pub fn model_path(&self, pos: Position) -> Result<Option<ModelPath>, EditError> {
        let gap = pos.index(&self.doc)?;
        let (node, para) = paragraph(&self.doc, &self.outline, gap)?;
        let Owner::Top { section, block } = self.outline.nodes[node].owner else {
            return Ok(None);
        };
        let mut cells = Vec::new();
        let mut n = node;
        while let Some(cell) = self.outline.nodes[n]
            .parent
            .filter(|&c| self.outline.nodes[c].kind == AtomKind::Cell)
        {
            let row = self.outline.nodes[cell]
                .parent
                .ok_or(EditError::Irregular)?;
            let table = self.outline.nodes[row].parent.ok_or(EditError::Irregular)?;
            let index_in = |parent: usize, upto: usize, kinds: &[AtomKind]| {
                (parent + 1..upto)
                    .filter(|&m| {
                        self.outline.nodes[m].parent == Some(parent)
                            && kinds.contains(&self.outline.nodes[m].kind)
                    })
                    .count()
            };
            let blocks = [AtomKind::Paragraph, AtomKind::Table];
            cells.push((
                index_in(table, row, &[AtomKind::Row]),
                index_in(row, cell, &[AtomKind::Cell]),
                index_in(cell, n, &blocks),
            ));
            n = table;
        }
        cells.reverse();
        Ok(Some(ModelPath {
            section,
            block,
            cells,
            offset: gap - para.start,
        }))
    }
}

impl Editor<'_> {
    fn child(&self, parent: usize, k: usize, kinds: &[AtomKind]) -> Option<usize> {
        let end = self.outline.nodes[parent]
            .end
            .unwrap_or(self.outline.nodes.len());
        (parent + 1..end)
            .filter(|&m| {
                self.outline.nodes[m].parent == Some(parent)
                    && kinds.contains(&self.outline.nodes[m].kind)
            })
            .nth(k)
    }

    /// The position `path` names.
    pub fn position_at_path(&self, path: &ModelPath) -> Result<Position, EditError> {
        let missing = EditError::NotApplicable("the path names no paragraph");
        let blocks = &self
            .outline
            .sections
            .get(path.section)
            .ok_or(EditError::UnknownPosition)?
            .1;
        let mut n = *blocks.get(path.block).ok_or(EditError::UnknownPosition)?;
        for &(row, cell, block) in &path.cells {
            let r = self
                .child(n, row, &[AtomKind::Row])
                .ok_or(EditError::UnknownPosition)?;
            let c = self
                .child(r, cell, &[AtomKind::Cell])
                .ok_or(EditError::UnknownPosition)?;
            n = self
                .child(c, block, &[AtomKind::Paragraph, AtomKind::Table])
                .ok_or(EditError::UnknownPosition)?;
        }
        if self.outline.nodes[n].kind != AtomKind::Paragraph {
            return Err(missing);
        }
        self.position_of_block(BlockOffset {
            block: n,
            offset: path.offset,
        })
    }
}

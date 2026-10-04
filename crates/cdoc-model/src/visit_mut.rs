//! Mutable walkers, the counterparts of [`crate::visit`]: importers use them to repair a
//! document they assembled (dangling references, say) without re-deriving the containment
//! rules.

use crate::block::Block;
use crate::paragraph::Paragraph;
use crate::table::Table;
use crate::Document;

/// Every paragraph in `blocks`, descending into table cells.
pub fn paragraphs_in(blocks: &mut [Block], f: &mut dyn FnMut(&mut Paragraph)) {
    for block in blocks {
        match block {
            Block::Paragraph(p) => f(p),
            Block::Table(t) => {
                for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    paragraphs_in(&mut cell.blocks, f);
                }
            }
        }
    }
}

/// Every table in `blocks`, outermost first, including nested ones.
pub fn tables_in(blocks: &mut [Block], f: &mut dyn FnMut(&mut Table)) {
    for block in blocks {
        if let Block::Table(t) = block {
            f(t);
            for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                tables_in(&mut cell.blocks, f);
            }
        }
    }
}

impl Document {
    /// Every top-level block list: section bodies, headers, footers and footnote bodies.
    pub fn for_each_container_mut(&mut self, f: &mut dyn FnMut(&mut Vec<Block>)) {
        for section in &mut self.sections {
            let p = &mut section.properties;
            for set in [&mut p.headers, &mut p.footers] {
                for hf in [&mut set.default, &mut set.first, &mut set.even]
                    .into_iter()
                    .flatten()
                {
                    f(&mut hf.blocks);
                }
            }
            f(&mut section.blocks);
        }
        for footnote in &mut self.footnotes {
            f(&mut footnote.blocks);
        }
    }

    /// Every paragraph anywhere, mutably.
    pub fn for_each_paragraph_mut(&mut self, f: &mut dyn FnMut(&mut Paragraph)) {
        self.for_each_container_mut(&mut |blocks| paragraphs_in(blocks, f));
    }

    /// Every table anywhere, mutably.
    pub fn for_each_table_mut(&mut self, f: &mut dyn FnMut(&mut Table)) {
        self.for_each_container_mut(&mut |blocks| tables_in(blocks, f));
    }
}

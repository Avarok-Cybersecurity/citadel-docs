//! A caret line's provenance resolved to its paragraph: the region's block list found in the
//! outline, then the path followed down through tables, rows and cells.

use crate::error::EditError;
use crate::outline::{Outline, Owner};
use cdoc_crdt::{atom_at, Atom, AtomKind, Doc, Furniture, Id};
use cdoc_layout::{Provenance, Region};
use std::collections::HashMap;

/// Resolves provenances against one document's outline, remembering the block lists it has
/// read.
pub(super) struct Resolver<'a> {
    outline: &'a Outline,
    /// The header, footer and footnote nodes, by region.
    asides: HashMap<Region, usize>,
    /// The child blocks (rows, cells) of each container node read so far, in order.
    children: HashMap<usize, Vec<usize>>,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(doc: &'a Doc, outline: &'a Outline) -> Result<Self, EditError> {
        let mut asides = HashMap::new();
        for (n, node) in outline.nodes.iter().enumerate() {
            if !matches!(node.kind, AtomKind::HeaderFooter | AtomKind::Footnote) {
                continue;
            }
            let Some(at) = doc.locate(node.id).filter(|l| l.visible) else {
                continue;
            };
            let region = match (atom_at(doc, at.index)?, node.owner) {
                (Some(Atom::Footnote(id)), _) => Region::Footnote(id),
                (Some(Atom::HeaderFooter { furniture, kind }), Owner::Furniture(s)) => {
                    let section = u32::try_from(s).unwrap_or(u32::MAX);
                    match furniture {
                        Furniture::Header => Region::Header { section, kind },
                        Furniture::Footer => Region::Footer { section, kind },
                    }
                }
                _ => continue,
            };
            asides.entry(region).or_insert(n);
        }
        Ok(Self {
            outline,
            asides,
            children: HashMap::new(),
        })
    }

    /// The nodes whose parent is `n`, other than ends: a container's blocks, a table's rows,
    /// a row's cells. One pass over `n`'s subtree (a table, header, footer or footnote, up to
    /// its end) lists every container inside it, so a row or a cell, which has no end of its
    /// own, is always listed by its table's pass first.
    fn children(&mut self, n: usize) -> &[usize] {
        if !self.children.contains_key(&n) {
            let nodes = &self.outline.nodes;
            let end = nodes[n].end.unwrap_or(n);
            self.children.insert(n, Vec::new());
            for (k, node) in nodes.iter().enumerate().take(end + 1).skip(n + 1) {
                self.children.entry(k).or_default();
                if let (Some(p), false) = (node.parent, node.kind == AtomKind::End) {
                    self.children.entry(p).or_default().push(k);
                }
            }
        }
        &self.children[&n]
    }

    /// The paragraph a line of `from` belongs to: its id. `None` only when the outline is
    /// irregular (the layout then lays out a repaired document the outline cannot follow).
    pub(super) fn paragraph(&mut self, from: &Provenance) -> Result<Option<Id>, EditError> {
        let found = self.find(from);
        match found {
            Some(n) if self.outline.nodes[n].kind == AtomKind::Paragraph => {
                Ok(Some(self.outline.nodes[n].id))
            }
            _ if !self.outline.regular => Ok(None),
            _ => Err(EditError::Unplaced),
        }
    }

    fn find(&mut self, from: &Provenance) -> Option<usize> {
        let (&first, rest) = from.path.split_first()?;
        let first = usize::try_from(first).ok()?;
        let mut node = match from.region {
            Region::Body { section } => {
                let section = usize::try_from(section).ok()?;
                *self.outline.sections.get(section)?.1.get(first)?
            }
            other => {
                let aside = *self.asides.get(&other)?;
                *self.children(aside).get(first)?
            }
        };
        for step in rest.chunks(3) {
            let &[row, cell, block] = step else {
                return None;
            };
            let row = *self.children(node).get(usize::try_from(row).ok()?)?;
            let cell = *self.children(row).get(usize::try_from(cell).ok()?)?;
            node = *self.children(cell).get(usize::try_from(block).ok()?)?;
        }
        Some(node)
    }
}

//! The document's block structure, read from the CRDT's list of structural atoms: which
//! top-level block of which section each atom belongs to, what container (cell, header,
//! footnote, section body) holds it, and where each table ends. Node `n` is block `n` in
//! [`cdoc_crdt::Changes`]' numbering, so a change report maps to layout blocks without a walk.
//!
//! The walk follows the grammar on [`cdoc_crdt::Atom`]. A sequence outside it (only concurrent
//! structural edits produce one, and `to_model` repairs it) is marked irregular: the editor
//! then converts and lays out the whole document after each change and refuses structural
//! commands until the structure is regular again.

use cdoc_crdt::{AtomKind, Id, Structural};

/// What a structural atom belongs to, for the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    /// Top-level block `block` of section `section` (or something inside it).
    Top { section: usize, block: usize },
    /// The start atom of section `section`.
    Section(usize),
    /// A header or footer of section `section`.
    Furniture(usize),
    /// A footnote body.
    Footnote,
}

/// One structural atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) id: Id,
    pub(crate) kind: AtomKind,
    pub(crate) inlines: bool,
    /// The container: the cell, header, footer, footnote or section node whose blocks hold
    /// this one; for a row its table, for a cell its row, for an end what it closes.
    pub(crate) parent: Option<usize>,
    pub(crate) owner: Owner,
    /// For a table, header, footer or footnote: its closing end node.
    pub(crate) end: Option<usize>,
}

/// The structure of the whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outline {
    pub(crate) nodes: Vec<Node>,
    /// Per section: its node, and its top-level blocks' nodes in order.
    pub(crate) sections: Vec<(usize, Vec<usize>)>,
    pub(crate) regular: bool,
}

#[derive(Clone, Copy)]
enum Frame {
    Table(usize),
    Row(usize),
    Cell(usize),
    Closable(usize),
}

impl Outline {
    pub(crate) fn build(structure: &[Structural]) -> Self {
        let mut out = Self {
            nodes: Vec::with_capacity(structure.len()),
            sections: Vec::new(),
            regular: true,
        };
        let mut stack: Vec<Frame> = Vec::new();
        let mut footnotes = false;
        for (n, s) in structure.iter().enumerate() {
            let ok = out.step(n, s, &mut stack, &mut footnotes);
            out.regular &= ok && (!s.inlines || s.kind == AtomKind::Paragraph);
        }
        out.regular &= stack.is_empty() && !out.sections.is_empty();
        out
    }

    fn push(&mut self, s: &Structural, parent: Option<usize>, owner: Owner) {
        self.nodes.push(Node {
            id: s.id,
            kind: s.kind,
            inlines: s.inlines,
            parent,
            owner,
            end: None,
        });
    }

    /// The container and owner a block opened now gets.
    fn place(&mut self, stack: &[Frame]) -> Option<(usize, Owner)> {
        let section = self.sections.len().checked_sub(1)?;
        match stack.last() {
            None => {
                let blocks = &mut self.sections[section].1;
                let owner = Owner::Top {
                    section,
                    block: blocks.len(),
                };
                blocks.push(self.nodes.len());
                Some((self.sections[section].0, owner))
            }
            Some(Frame::Cell(c) | Frame::Closable(c)) => Some((*c, self.nodes[*c].owner)),
            Some(Frame::Table(_) | Frame::Row(_)) => None,
        }
    }

    fn step(&mut self, n: usize, s: &Structural, stack: &mut Vec<Frame>, notes: &mut bool) -> bool {
        let section = self.sections.len().checked_sub(1);
        match s.kind {
            AtomKind::Section => {
                let fresh = stack.is_empty() && !*notes;
                stack.clear();
                self.sections.push((n, Vec::new()));
                self.push(s, None, Owner::Section(self.sections.len() - 1));
                fresh
            }
            AtomKind::HeaderFooter | AtomKind::Footnote => {
                let Some(section) = section.filter(|_| stack.is_empty()) else {
                    return false;
                };
                // Furniture after a body block would sit between blocks of the body.
                let between =
                    s.kind == AtomKind::HeaderFooter && !self.sections[section].1.is_empty();
                let owner = if s.kind == AtomKind::Footnote {
                    *notes = true;
                    Owner::Footnote
                } else {
                    Owner::Furniture(section)
                };
                self.push(s, Some(self.sections[section].0), owner);
                stack.push(Frame::Closable(n));
                !between
            }
            AtomKind::Paragraph | AtomKind::Table => {
                let Some((parent, owner)) = self.place(stack) else {
                    return false;
                };
                self.push(s, Some(parent), owner);
                if s.kind == AtomKind::Table {
                    stack.push(Frame::Table(n));
                }
                true
            }
            AtomKind::Row => {
                while matches!(stack.last(), Some(Frame::Cell(_) | Frame::Row(_))) {
                    stack.pop();
                }
                let Some(&Frame::Table(t)) = stack.last() else {
                    return false;
                };
                self.push(s, Some(t), self.nodes[t].owner);
                stack.push(Frame::Row(n));
                true
            }
            AtomKind::Cell => {
                if matches!(stack.last(), Some(Frame::Cell(_))) {
                    stack.pop();
                }
                let Some(&Frame::Row(r)) = stack.last() else {
                    return false;
                };
                self.push(s, Some(r), self.nodes[r].owner);
                stack.push(Frame::Cell(n));
                true
            }
            AtomKind::End => {
                while matches!(stack.last(), Some(Frame::Cell(_) | Frame::Row(_))) {
                    stack.pop();
                }
                let Some(Frame::Table(open) | Frame::Closable(open)) = stack.pop() else {
                    return false;
                };
                self.push(s, Some(open), self.nodes[open].owner);
                self.nodes[open].end = Some(n);
                true
            }
            AtomKind::Inline => false,
        }
    }

    /// The nodes of the top-level blocks of `section`, by their ids.
    pub(crate) fn block_ids(&self, section: usize) -> Vec<Id> {
        self.sections[section]
            .1
            .iter()
            .map(|&n| self.nodes[n].id)
            .collect()
    }

    /// The first node after the subtree that starts at node `n`.
    pub(crate) fn after_subtree(&self, n: usize) -> usize {
        self.nodes[n].end.unwrap_or(n) + 1
    }
}

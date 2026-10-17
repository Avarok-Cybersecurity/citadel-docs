//! Footnotes nothing in the sections reaches: ODF has no note without a mark, so they are
//! written as notes in the first body paragraph, and the sidecar tells the reader to drop
//! their marks.

use super::line::Line;
use super::Writer;
use cdoc_model::{Block, FootnoteId, InlineContent};
use cdoc_pkg::xml::{Child, Node};
use cdoc_pkg::PackageError;
use std::collections::BTreeSet;

fn refs(blocks: &[Block]) -> impl Iterator<Item = FootnoteId> + '_ {
    blocks
        .iter()
        .flat_map(Block::paragraphs)
        .flat_map(|p| &p.inlines)
        .filter_map(|i| match i.content {
            InlineContent::FootnoteRef { id } => Some(id),
            _ => None,
        })
}

impl Writer<'_> {
    /// Footnotes the sections refer to, directly or through other such footnotes.
    fn reachable_footnotes(&self) -> BTreeSet<FootnoteId> {
        let mut todo: Vec<FootnoteId> = Vec::new();
        for section in &self.doc.sections {
            let p = &section.properties;
            todo.extend(refs(&section.blocks));
            for (_, hf) in p.headers.present().into_iter().chain(p.footers.present()) {
                todo.extend(refs(&hf.blocks));
            }
        }
        let mut seen = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if seen.insert(id) {
                if let Some(note) = self.doc.footnotes.iter().find(|f| f.id == id) {
                    todo.extend(refs(&note.blocks));
                }
            }
        }
        seen
    }

    /// Notes for footnotes nothing refers to, placed in the first body paragraph and marked in
    /// the sidecar so the reader drops their marks.
    pub(super) fn orphan_footnotes(&mut self) -> Result<Vec<Node>, PackageError> {
        let referenced = self.reachable_footnotes();
        let orphans: Vec<FootnoteId> = self
            .doc
            .footnotes
            .iter()
            .map(|f| f.id)
            .filter(|id| !referenced.contains(id))
            .collect();
        let mut out = Vec::new();
        for id in orphans {
            if self.footnotes_written.contains(&id) {
                continue;
            }
            self.log
                .lose("a footnote nothing refers to written as a note in the first paragraph");
            self.sidecar.orphans.insert(id);
            let mut line = Line::new();
            self.footnote(id, &mut line)?;
            out.extend(line.finish().into_iter().filter_map(|c| match c {
                Child::Node(n) => Some(n),
                Child::Text(_) => None,
            }));
        }
        Ok(out)
    }
}

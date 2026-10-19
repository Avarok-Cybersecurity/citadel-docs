//! Stories: the body, and each header, footer and footnote body, which a selection never
//! leaves. A header, footer or footnote is a container in the flat document (its start atom,
//! its blocks, its end atom); every caret gap outside every container is the body's.

use crate::error::EditError;
use crate::outline::{Outline, Owner};
use crate::position::{is_caret_gap, Direction};
use cdoc_crdt::{atom_at, Atom, Doc, Furniture};
use cdoc_model::{FootnoteId, HeaderFooterKind};
use std::ops::RangeInclusive;

/// Where a position is: the body or one header, footer or footnote body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Story {
    /// The sections' body text.
    Body,
    /// A section's header.
    Header {
        /// The section.
        section: u32,
        /// Which of its headers.
        kind: HeaderFooterKind,
    },
    /// A section's footer.
    Footer {
        /// The section.
        section: u32,
        /// Which of its footers.
        kind: HeaderFooterKind,
    },
    /// A footnote's body.
    Footnote(FootnoteId),
}

/// A header, footer or footnote body: the gaps inside it, from after its start atom to
/// before its end atom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Container {
    pub(crate) story: Story,
    pub(crate) gaps: RangeInclusive<usize>,
}

/// Every header, footer and footnote body in document order.
pub(crate) fn containers(doc: &Doc, outline: &Outline) -> Result<Vec<Container>, EditError> {
    let mut out = Vec::new();
    for node in outline.containers.iter().map(|&n| &outline.nodes[n]) {
        let (Some(start), Some(end)) = (
            doc.locate(node.id).filter(|l| l.visible),
            node.end
                .and_then(|e| doc.locate(outline.nodes[e].id))
                .filter(|l| l.visible),
        ) else {
            continue;
        };
        let story = match (atom_at(doc, start.index)?, node.owner) {
            (Some(Atom::Footnote(id)), _) => Story::Footnote(id),
            (Some(Atom::HeaderFooter { furniture, kind }), Owner::Furniture(s)) => {
                let section = u32::try_from(s).map_err(|_| EditError::Irregular)?;
                match furniture {
                    Furniture::Header => Story::Header { section, kind },
                    Furniture::Footer => Story::Footer { section, kind },
                }
            }
            _ => continue,
        };
        out.push(Container {
            story,
            gaps: start.index + 1..=end.index,
        });
    }
    Ok(out)
}

/// The stories of one document, read once.
pub(crate) struct Stories<'d> {
    doc: &'d Doc,
    containers: Vec<Container>,
}

impl<'d> Stories<'d> {
    pub(crate) fn new(doc: &'d Doc, outline: &Outline) -> Result<Self, EditError> {
        Ok(Self {
            doc,
            containers: containers(doc, outline)?,
        })
    }

    fn container(&self, gap: usize) -> Option<&Container> {
        self.containers.iter().find(|c| c.gaps.contains(&gap))
    }

    /// The story gap `gap` is in.
    pub(crate) fn at(&self, gap: usize) -> Story {
        self.container(gap).map_or(Story::Body, |c| c.story)
    }

    /// The caret gaps of `story`, first and last; `None` when it has none.
    pub(crate) fn bounds(&self, story: Story) -> Option<(usize, usize)> {
        let gaps = match self.containers.iter().find(|c| c.story == story) {
            Some(c) => c.gaps.clone(),
            None if story == Story::Body => 0..=self.doc.len(),
            None => return None,
        };
        let caret = |g: &usize| is_caret_gap(self.doc, *g) && self.at(*g) == story;
        let first = gaps.clone().find(caret)?;
        let last = gaps.rev().find(caret)?;
        Some((first, last))
    }

    /// `gap` moved into `story`: itself when it is there, else the story's nearest caret gap,
    /// looking first the way `dir` says.
    pub(crate) fn clamp(&self, story: Story, gap: usize, dir: Direction) -> Option<usize> {
        if is_caret_gap(self.doc, gap) && self.at(gap) == story {
            return Some(gap);
        }
        let inside = |g: usize| is_caret_gap(self.doc, g) && self.at(g) == story;
        let before = || (0..=gap.min(self.doc.len())).rev().find(|&g| inside(g));
        let after = || (gap..=self.doc.len()).find(|&g| inside(g));
        match dir {
            Direction::Backward => before().or_else(after),
            Direction::Forward => after().or_else(before),
        }
    }
}

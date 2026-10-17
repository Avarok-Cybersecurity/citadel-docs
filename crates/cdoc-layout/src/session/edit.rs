//! The previous layout's records brought up to date with the edits: fingerprints for the new
//! blocks, the boundary to resume placement from, and the first boundary after every change.

use super::dirty::{BlockEdit, Dirty};
use super::hash::fingerprint;
use super::record::{BlockRecord, Boundary, SectionRecord};
use crate::context::Trace;
use cdoc_model::{Block, Document, FootnoteId, InlineContent};

/// A resume point: block `index` of `section` (the section's length for its end), in the
/// state the previous layout recorded there.
pub struct ResumeAt {
    /// The section.
    pub section: usize,
    /// The block.
    pub index: usize,
    /// The state.
    pub boundary: Boundary,
    /// The first position (section, index) after every changed block: the earliest the
    /// placement can find itself where the previous one was.
    pub clean_from: (usize, usize),
}

/// Where placement starts.
pub enum Start {
    /// From the top of the document, with nothing reused but measurements.
    Top,
    /// From a recorded boundary.
    At(Box<ResumeAt>),
    /// Nowhere: no block changed.
    Nowhere,
}

/// The footnotes a block references, in order.
fn refs(block: &Block) -> Vec<FootnoteId> {
    block
        .paragraphs()
        .flat_map(|p| &p.inlines)
        .filter_map(|i| match i.content {
            InlineContent::FootnoteRef { id } => Some(id),
            _ => None,
        })
        .collect()
}

/// A record for a block not yet measured.
fn fresh(block: &Block) -> BlockRecord {
    BlockRecord {
        content: fingerprint(block),
        refs: refs(block),
        measured: None,
        at: None,
        place: Trace::empty(),
    }
}

/// Records for every block of the document.
pub fn all_fresh(doc: &Document) -> Vec<SectionRecord> {
    doc.sections
        .iter()
        .map(|s| SectionRecord {
            blocks: s.blocks.iter().map(fresh).collect(),
            start: Trace::empty(),
            end: None,
        })
        .collect()
}

/// The boundary in front of old block `at` of `section`, moved back over blocks kept with the
/// next (whose chain reaches the change).
fn resume_point(old: &[SectionRecord], edit: &BlockEdit) -> Option<(usize, Boundary)> {
    let blocks = &old[edit.section].blocks;
    let mut at = edit.at;
    while at > 0
        && blocks[at - 1]
            .measured
            .as_ref()
            .is_some_and(|m| m.block.keep_with_next)
    {
        at -= 1;
    }
    let boundary = match blocks.get(at) {
        Some(record) => record.at.clone(),
        None => old[edit.section].end.clone(),
    }?;
    Some((at, boundary))
}

/// Applies `dirty`'s edits to `old`, fingerprinting the inserted blocks from `doc`, and says
/// where placement must start. `incremental` false asks for [`Start::Top`].
pub fn apply(
    mut old: Vec<SectionRecord>,
    doc: &Document,
    dirty: &Dirty,
    incremental: bool,
) -> (Vec<SectionRecord>, Start) {
    if dirty.everything || old.len() != doc.sections.len() {
        return (all_fresh(doc), Start::Top);
    }
    let resume = dirty
        .edits
        .first()
        .and_then(|first| resume_point(&old, first).map(|r| (first.section, r)));
    let mut clean_from = (0, 0);
    let mut shift = vec![0isize; old.len()];
    let mut placed: Vec<(BlockEdit, usize)> = Vec::with_capacity(dirty.edits.len());
    for edit in &dirty.edits {
        let new_at = edit.at.checked_add_signed(shift[edit.section]).unwrap_or(0);
        placed.push((*edit, new_at));
        clean_from = (edit.section, new_at + edit.inserted);
        shift[edit.section] += isize::try_from(edit.inserted).unwrap_or(0)
            - isize::try_from(edit.removed).unwrap_or(0);
    }
    for (edit, new_at) in placed.into_iter().rev() {
        let blocks = &doc.sections[edit.section].blocks[new_at..new_at + edit.inserted];
        old[edit.section]
            .blocks
            .splice(edit.at..edit.at + edit.removed, blocks.iter().map(fresh));
    }
    let start = match resume {
        _ if !incremental => Start::Top,
        None if dirty.edits.is_empty() => Start::Nowhere,
        None => Start::Top,
        Some((section, (index, boundary))) => Start::At(Box::new(ResumeAt {
            section,
            index,
            boundary,
            clean_from,
        })),
    };
    (old, start)
}

//! Keeping the model view and the layout in step with the CRDT after an edit, local or remote:
//! which top-level blocks changed (from the units an edit touched, or the blocks an update
//! reports), the layout's [`Dirty`] for them, and the model's blocks re-read from the CRDT for
//! exactly those. Anything outside the body (headers, footers, footnotes, section properties,
//! document attributes) re-reads the whole model, which is rare and still incremental for the
//! layout.

use crate::error::EditError;
use crate::outline::{Outline, Owner};
use cdoc_crdt::{fold_blocks, slice, to_model, Doc, Id};
use cdoc_layout::{BlockEdit, Dirty};
use cdoc_model::{Block, Document};
use std::collections::BTreeSet;
use std::ops::Range;

/// The block ordinals (numbered as in `cdoc_crdt::Changes`) a touched span dirties: from the
/// block holding the gap before its first unit (so a new boundary dirties the block it split)
/// to the block holding its last unit.
pub(crate) fn ordinals(doc: &Doc, spans: &[(Id, Id)]) -> Vec<Range<usize>> {
    spans
        .iter()
        .filter_map(|&(a, b)| {
            let (a, b) = (doc.locate(a)?, doc.locate(b)?);
            let first = doc.blocks_before(a.index).saturating_sub(1);
            let last = doc
                .blocks_before(b.index + usize::from(b.visible))
                .saturating_sub(1);
            Some(first.min(last)..first.max(last) + 1)
        })
        .collect()
}

#[derive(Default)]
struct Touched {
    /// Per section, the ids of changed top-level blocks.
    blocks: Vec<BTreeSet<Id>>,
    section: bool,
    furniture: bool,
    footnotes: bool,
}

fn classify(outline: &Outline, ranges: &[Range<usize>]) -> Touched {
    let mut t = Touched {
        blocks: vec![BTreeSet::new(); outline.sections.len()],
        ..Touched::default()
    };
    for range in ranges {
        for node in outline.nodes.get(range.clone()).unwrap_or(&[]) {
            match node.owner {
                Owner::Top { section, block } => {
                    let top = outline.sections[section].1[block];
                    t.blocks[section].insert(outline.nodes[top].id);
                }
                Owner::Section(_) | Owner::Stray => t.section = true,
                Owner::Furniture(_) => t.furniture = true,
                Owner::Footnote => t.footnotes = true,
            }
        }
    }
    t
}

/// The block edits turning `old` into `new`, where `changed` blocks changed in place; `None`
/// if the two orders disagree (which a list CRDT never produces).
fn diff(section: usize, old: &[Id], new: &[Id], changed: &BTreeSet<Id>) -> Option<Vec<BlockEdit>> {
    let (in_old, in_new): (BTreeSet<Id>, BTreeSet<Id>) =
        (old.iter().copied().collect(), new.iter().copied().collect());
    let mut edits = Vec::new();
    // The open edit: where it starts, blocks removed, blocks inserted.
    let mut cur: Option<(usize, usize, usize)> = None;
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        let (removed, inserted) = if i < old.len() && j < new.len() && old[i] == new[j] {
            if !changed.contains(&old[i]) {
                edits.extend(cur.take());
                (i, j) = (i + 1, j + 1);
                continue;
            }
            (1, 1)
        } else if i < old.len() && !in_new.contains(&old[i]) {
            (1, 0)
        } else if j < new.len() && !in_old.contains(&new[j]) {
            (0, 1)
        } else {
            return None;
        };
        let e = cur.get_or_insert((i, 0, 0));
        (e.1, e.2) = (e.1 + removed, e.2 + inserted);
        (i, j) = (i + removed, j + inserted);
    }
    edits.extend(cur);
    Some(
        edits
            .into_iter()
            .map(|(at, removed, inserted)| BlockEdit {
                section,
                at,
                removed,
                inserted,
            })
            .collect(),
    )
}

/// The visible units of top-level blocks `blocks` (node ordinals, consecutive in a section).
fn units_of(doc: &Doc, outline: &Outline, all: &[usize], range: Range<usize>) -> Range<usize> {
    let at = |n: usize| {
        outline
            .nodes
            .get(n)
            .and_then(|node| doc.locate(node.id))
            .map_or(doc.len(), |l| l.index)
    };
    let start = at(all[range.start]);
    let end = match all.get(range.end) {
        Some(&n) => at(n),
        None => at(outline.after_subtree(all[range.end - 1])),
    };
    start..end
}

/// Brings `model` up to date and says what the layout must redo.
pub(crate) fn sync(
    doc: &Doc,
    model: &mut Document,
    old: &Outline,
    new: &Outline,
    touched: &[Range<usize>],
    attrs: &[String],
) -> Result<Dirty, EditError> {
    if !old.regular || !new.regular || old.sections.len() != new.sections.len() {
        *model = to_model(doc)?;
        return Ok(Dirty::all());
    }
    let mut t = classify(new, touched);
    // A deleted unit dirties the block before it, which says nothing of the container it was
    // in: compare the structure outside the bodies directly.
    let outside = |o: &Outline, f: fn(Owner) -> bool| -> Vec<Id> {
        o.nodes
            .iter()
            .filter(|n| f(n.owner))
            .map(|n| n.id)
            .collect()
    };
    t.footnotes |= outside(old, |o| o == Owner::Footnote) != outside(new, |o| o == Owner::Footnote);
    t.furniture |= outside(old, |o| matches!(o, Owner::Furniture(_)))
        != outside(new, |o| matches!(o, Owner::Furniture(_)));
    let mut edits = Vec::new();
    for (s, changed) in t.blocks.iter().enumerate() {
        let (before, after) = (old.block_ids(s), new.block_ids(s));
        let Some(found) = diff(s, &before, &after, changed) else {
            *model = to_model(doc)?;
            return Ok(Dirty::all());
        };
        edits.extend(found);
    }
    let whole = t.section || t.furniture || t.footnotes || !attrs.is_empty();
    if whole {
        *model = to_model(doc)?;
    } else {
        for e in edits.iter().rev() {
            let all = &new.sections[e.section].1;
            let start = new_start(&edits, e);
            let blocks: Vec<Block> = if e.inserted == 0 {
                Vec::new()
            } else {
                let units = units_of(doc, new, all, start..start + e.inserted);
                fold_blocks(slice(doc, units)?)
            };
            model.sections[e.section]
                .blocks
                .splice(e.at..e.at + e.removed, blocks);
        }
    }
    let has = |name: &str| attrs.iter().any(|a| a == name);
    let mut dirty = Dirty::edits(edits);
    dirty.styles_changed = has(cdoc_crdt::names::STYLES) || has(cdoc_crdt::names::SETTINGS);
    dirty.numbering_changed = has(cdoc_crdt::names::NUMBERING);
    dirty.page_setup_changed = t.section;
    dirty.furniture_changed = t.furniture;
    dirty.footnotes_changed = t.footnotes;
    Ok(dirty)
}

/// Where edit `e` starts in the new document: moved by the earlier edits of its section.
fn new_start(edits: &[BlockEdit], e: &BlockEdit) -> usize {
    let mut shift = 0isize;
    for p in edits.iter().take_while(|p| !std::ptr::eq(*p, e)) {
        if p.section == e.section {
            shift += p.inserted.cast_signed() - p.removed.cast_signed();
        }
    }
    e.at.saturating_add_signed(shift)
}

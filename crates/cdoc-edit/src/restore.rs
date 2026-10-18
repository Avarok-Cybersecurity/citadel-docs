//! Putting formatting and content back exactly: the inverse of every [`Step`], and the two
//! primitives it needs, "make this range's formatting exactly this" and "insert these pieces
//! here". Paste and mark inheritance use the same primitives.

use crate::error::EditError;
use crate::run_keys::{fields, mark_edits, run_marks};
use crate::tx::{piece_len, Segment, Step, Target, Tx};
use cdoc_crdt::{slice, Atom, DocAttr, Id, Mark, Piece};
use cdoc_model::{Document, InlineContent, Marks, RunProperties};
use std::ops::Range;

/// The marks that put `target` back to what `seg` recorded, over the whole segment.
fn previous(target: &Target, seg: &Segment) -> Vec<Mark> {
    match target {
        Target::Run(keys) => {
            let Some(props) = &seg.props else {
                return Vec::new();
            };
            fields(props)
                .into_iter()
                .filter(|(k, _)| keys.contains(k))
                .map(|(k, v)| {
                    if v.is_none() {
                        Mark::ClearRun(k)
                    } else {
                        Mark::Run(v)
                    }
                })
                .collect()
        }
        Target::Link => vec![Mark::Link(seg.marks.link.clone())],
        Target::Revision => vec![Mark::Revision(seg.marks.revision.clone())],
        Target::Comment(id) => vec![Mark::Comment {
            id: id.clone(),
            anchored: seg.marks.comments.contains(id),
        }],
    }
}

/// Makes the formatting of `range` exactly `props` (characters only) and, when given, exactly
/// `marks`. Each run's edit reaches to the next run, over structural atoms between them, so a
/// mark removed from the range leaves no gap a later insertion could fall into.
pub(crate) fn set_exact(
    tx: &mut Tx<'_>,
    range: Range<usize>,
    props: Option<&RunProperties>,
    marks: Option<&Marks>,
    record: bool,
) -> Result<(), EditError> {
    let mut runs = Vec::new();
    let mut at = range.start;
    for piece in slice(tx.doc, range.clone())? {
        let len = piece_len(&piece);
        if let Piece::Inline(inline) = &piece {
            let mut edits = Vec::new();
            if let (InlineContent::Text(run), Some(want)) = (&inline.content, props) {
                edits.extend(run_marks(&run.properties, want));
            }
            if let Some(want) = marks {
                edits.extend(mark_edits(&inline.marks, want));
            }
            runs.push((at, edits));
        }
        at += len;
    }
    let ends: Vec<usize> = runs
        .iter()
        .skip(1)
        .map(|r| r.0)
        .chain([range.end])
        .collect();
    for ((start, edits), end) in runs.into_iter().zip(ends) {
        for mark in edits {
            tx.format(start..end, &mark, record)?;
        }
    }
    Ok(())
}

/// Inserts `pieces` at gap `index` with exactly their formatting; returns the gap after them.
pub(crate) fn insert_pieces(
    tx: &mut Tx<'_>,
    mut index: usize,
    pieces: &[Piece],
) -> Result<usize, EditError> {
    for piece in pieces {
        let len = piece_len(piece);
        match piece {
            Piece::Inline(inline) => {
                let props = match &inline.content {
                    InlineContent::Text(run) => {
                        tx.insert_text(index, &run.text)?;
                        Some(&run.properties)
                    }
                    content => {
                        tx.insert_atom(index, &Atom::Inline(content.clone()))?;
                        None
                    }
                };
                set_exact(tx, index..index + len, props, Some(&inline.marks), false)?;
            }
            Piece::Block(atom) => tx.insert_atom(index, atom)?,
        }
        index += len;
    }
    Ok(index)
}

/// The visible units among `runs`, as ascending index ranges.
fn visible_ranges(tx: &Tx<'_>, runs: &[(Id, u64)]) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let ids = runs
        .iter()
        .flat_map(|&(f, n)| (0..n).map(move |k| Id::new(f.client, f.clock + k)));
    for id in ids {
        let Some(at) = tx.doc.locate(id) else {
            continue;
        };
        if !at.visible {
            continue;
        }
        match out.last_mut() {
            Some(r) if r.end == at.index => r.end += 1,
            _ => out.push(at.index..at.index + 1),
        }
    }
    out.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in out {
        match merged.last_mut() {
            Some(m) if m.end == r.start => m.end = r.end,
            _ => merged.push(r),
        }
    }
    merged
}

fn span(tx: &Tx<'_>, first: Id, last: Id) -> Option<Range<usize>> {
    let a = tx.doc.locate(first)?;
    let b = tx.doc.locate(last)?;
    let end = b.index + usize::from(b.visible);
    (a.index < end).then_some(a.index..end)
}

/// Undoes one step through `tx`, which records the step that redoes it.
pub(crate) fn invert(tx: &mut Tx<'_>, step: &Step, model: &Document) -> Result<(), EditError> {
    match step {
        Step::Inserted { runs } => {
            for range in visible_ranges(tx, runs).into_iter().rev() {
                tx.delete(range)?;
            }
        }
        Step::Deleted { after, ids, pieces } => {
            let gap = crate::position::Position::after(*after).index(tx.doc)?;
            let from = tx.fresh.len();
            insert_pieces(tx, gap, pieces)?;
            let copies: Vec<Id> = tx.fresh[from..].to_vec();
            tx.remap.extend(ids.iter().copied().zip(copies));
        }
        Step::Formatted { target, segments } => {
            // Adjacent segments that go back to the same value are restored as one range: a
            // non-expanding mark (a link, a comment) set over two touching ranges leaves the
            // gap between them uncovered, where a later insertion would keep the old value.
            let mut groups: Vec<(Id, Id, Vec<Mark>)> = Vec::new();
            for seg in segments {
                let marks = previous(target, seg);
                match groups.last_mut() {
                    Some(g) if g.2 == marks => g.1 = seg.last,
                    _ => groups.push((seg.first, seg.last, marks)),
                }
            }
            for (first, last, marks) in groups {
                if let Some(range) = span(tx, first, last) {
                    for mark in marks {
                        tx.format(range.clone(), &mark, true)?;
                    }
                }
            }
        }
        Step::Block { atom, previous } => {
            if let Some(at) = tx.doc.locate(*atom).filter(|a| a.visible) {
                tx.set_block(at.index, previous)?;
            }
        }
        Step::Attr { previous } => {
            tx.set_attr(previous, current_attr(model, previous))?;
        }
    }
    Ok(())
}

/// The model's current value of the attribute `like` names.
pub(crate) fn current_attr(model: &Document, like: &DocAttr) -> DocAttr {
    match like {
        DocAttr::Meta(_) => DocAttr::Meta(model.meta.clone()),
        DocAttr::Settings(_) => DocAttr::Settings(model.settings.clone()),
        DocAttr::Styles(_) => DocAttr::Styles(model.styles.clone()),
        DocAttr::Numbering(_) => DocAttr::Numbering(model.numbering.clone()),
        DocAttr::Comments(_) => DocAttr::Comments(model.comments.clone()),
    }
}

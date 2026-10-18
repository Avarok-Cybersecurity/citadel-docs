//! One transaction: the primitive edits a command makes, each recorded with what undoes it and
//! with the units it touched (for the layout). Undo replays inverse steps through a new
//! transaction, which records the steps that redo them, so undo and redo are one mechanism.

use crate::error::EditError;
use cdoc_crdt::{atom_at, slice, Atom, Doc, DocAttr, Id, Mark, Piece};
use cdoc_model::InlineContent;
use std::ops::Range;

pub(crate) use crate::step::{Segment, Step, Target};

/// A transaction over the replica.
pub(crate) struct Tx<'d> {
    pub(crate) doc: &'d mut Doc,
    pub(crate) steps: Vec<Step>,
    /// Spans of units touched, by first and last id (deleted units keep their place).
    pub(crate) touched: Vec<(Id, Id)>,
    /// Document attributes written.
    pub(crate) attrs: Vec<&'static str>,
    /// A structural atom was inserted or deleted since the outline was last built.
    pub(crate) structural: bool,
    /// Every unit inserted, in order of insertion.
    pub(crate) fresh: Vec<Id>,
    /// Deleted units that undo brought back, with the ids of their copies.
    pub(crate) remap: Vec<(Id, Id)>,
}

/// Adds the run `first..first + len` to `runs`, extending the last run when it continues it.
pub(crate) fn add_run(runs: &mut Vec<(Id, u64)>, first: Id, len: u64) {
    match runs.last_mut() {
        Some((f, l)) if f.client == first.client && f.clock + *l == first.clock => *l += len,
        _ => runs.push((first, len)),
    }
}

/// The units a piece covers.
pub(crate) fn piece_len(piece: &Piece) -> usize {
    match piece {
        Piece::Inline(inline) => match &inline.content {
            InlineContent::Text(run) => run.text.chars().count(),
            _ => 1,
        },
        Piece::Block(_) => 1,
    }
}

impl<'d> Tx<'d> {
    pub(crate) fn new(doc: &'d mut Doc) -> Self {
        Self {
            doc,
            steps: Vec::new(),
            touched: Vec::new(),
            attrs: Vec::new(),
            structural: false,
            fresh: Vec::new(),
            remap: Vec::new(),
        }
    }

    fn inserted(&mut self, first: Id) {
        let len = self.doc.next_id().clock - first.clock;
        let last = Id::new(first.client, first.clock + len - 1);
        self.touched.push((first, last));
        self.fresh
            .extend((0..len).map(|k| Id::new(first.client, first.clock + k)));
        match self.steps.last_mut() {
            Some(Step::Inserted { runs }) => add_run(runs, first, len),
            _ => self.steps.push(Step::Inserted {
                runs: vec![(first, len)],
            }),
        }
    }

    /// Inserts text at gap `index`.
    pub(crate) fn insert_text(&mut self, index: usize, text: &str) -> Result<(), EditError> {
        let first = self.doc.next_id();
        self.doc.insert_text(index, text)?;
        self.inserted(first);
        Ok(())
    }

    /// Inserts an atom at gap `index`.
    pub(crate) fn insert_atom(&mut self, index: usize, atom: &Atom) -> Result<(), EditError> {
        let first = self.doc.next_id();
        self.doc.insert_atom(index, atom)?;
        self.structural |= !matches!(atom, Atom::Inline(_));
        self.inserted(first);
        Ok(())
    }

    fn id(&self, index: usize) -> Result<Id, EditError> {
        self.doc.id_at(index).ok_or(EditError::UnknownPosition)
    }

    /// Deletes the visible units `range`.
    pub(crate) fn delete(&mut self, range: Range<usize>) -> Result<(), EditError> {
        if range.is_empty() {
            return Ok(());
        }
        let pieces = slice(self.doc, range.clone())?;
        let after = self.id(range
            .start
            .checked_sub(1)
            .ok_or(EditError::UnknownPosition)?)?;
        let ids: Vec<Id> = self
            .doc
            .units_from(range.start)
            .take(range.len())
            .map(|(id, _)| id)
            .collect();
        let (at, last) = (ids[0], ids[ids.len() - 1]);
        self.structural |= pieces.iter().any(|p| matches!(p, Piece::Block(_)));
        self.doc.delete(range.start, range.len())?;
        self.touched.push((at, last));
        self.steps.push(Step::Deleted { after, ids, pieces });
        Ok(())
    }

    /// The formatting of `range`, run by run. The segments tile the range (the first starts at
    /// its start, each reaches to the next, over structural atoms between them), so restoring
    /// them covers all of it: a unit later inserted anywhere inside is restored too.
    pub(crate) fn segments(&self, range: Range<usize>) -> Result<Vec<Segment>, EditError> {
        let mut runs = Vec::new();
        let mut at = range.start;
        for piece in slice(self.doc, range.clone())? {
            let len = piece_len(&piece);
            if let Piece::Inline(inline) = piece {
                let props = match inline.content {
                    InlineContent::Text(run) => Some(run.properties),
                    _ => None,
                };
                runs.push((at, props, inline.marks));
            }
            at += len;
        }
        if let Some(first) = runs.first_mut() {
            first.0 = range.start;
        }
        let ends: Vec<usize> = runs
            .iter()
            .skip(1)
            .map(|r| r.0)
            .chain([range.end])
            .collect();
        runs.into_iter()
            .zip(ends)
            .map(|((start, props, marks), end)| {
                Ok(Segment {
                    first: self.id(start)?,
                    last: self.id(end - 1)?,
                    props,
                    marks,
                })
            })
            .collect()
    }

    /// Formats `range`; `record` is false for units this transaction inserted (deleting them
    /// undoes their formatting too).
    pub(crate) fn format(
        &mut self,
        range: Range<usize>,
        mark: &Mark,
        record: bool,
    ) -> Result<(), EditError> {
        if range.is_empty() {
            return Ok(());
        }
        if record {
            let segments = self.segments(range.clone())?;
            self.steps.push(Step::Formatted {
                target: Target::of(mark),
                segments,
            });
        }
        self.touched
            .push((self.id(range.start)?, self.id(range.end - 1)?));
        self.doc.format(range, mark)?;
        Ok(())
    }

    /// Replaces the properties of the structural atom at `index`.
    pub(crate) fn set_block(&mut self, index: usize, atom: &Atom) -> Result<(), EditError> {
        let previous = atom_at(self.doc, index)?.ok_or(EditError::UnknownPosition)?;
        if previous == *atom {
            return Ok(());
        }
        let id = self.id(index)?;
        self.doc.set_block(index, atom)?;
        self.touched.push((id, id));
        self.steps.push(Step::Block { atom: id, previous });
        Ok(())
    }

    /// Writes a document attribute whose current value is `previous`.
    pub(crate) fn set_attr(&mut self, attr: &DocAttr, previous: DocAttr) -> Result<(), EditError> {
        self.doc.set_attr(attr)?;
        self.attrs.push(attr.name());
        self.steps.push(Step::Attr { previous });
        Ok(())
    }
}

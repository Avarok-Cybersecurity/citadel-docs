//! Read access by visible index and by id, for an editor: which unit sits where, where a unit
//! (or the tombstone it left) sits now, and the structural atoms in order. Ids are stable
//! across concurrent edits, so an editor anchors its caret to them and converts to an index
//! only when it needs one.

use super::Doc;
use crate::id::Id;
use crate::item::{byte_prefix, AtomKind, Content};

/// One visible unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A character.
    Char(char),
    /// An atom (inline or structural).
    Atom(AtomKind),
}

/// Where a unit is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Located {
    /// For a visible unit its index; for a deleted one the index of the gap it left (the
    /// number of visible units before it).
    pub index: usize,
    /// Whether the unit is still visible.
    pub visible: bool,
}

/// A visible structural atom, as [`Doc::structure`] lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Structural {
    /// Its id.
    pub id: Id,
    /// Its kind.
    pub kind: AtomKind,
    /// Whether inline units (characters, inline atoms) follow it before the next structural
    /// atom: a paragraph with content, or inlines where the grammar has none.
    pub inlines: bool,
}

/// Visible units in document order, with their ids; see [`Doc::units_from`].
#[derive(Debug, Clone)]
pub struct Units<'d> {
    doc: &'d Doc,
    /// The item, the unit offset in it and that unit's byte offset in the item's text.
    at: Option<(u32, u64, usize)>,
}

fn to_usize(n: u64) -> usize {
    usize::try_from(n).expect("fits the address space")
}

impl Doc {
    /// The id the next unit this replica inserts will get.
    #[must_use]
    pub fn next_id(&self) -> Id {
        Id::new(self.client, self.next_clock(self.client))
    }

    /// The id of visible unit `index`.
    #[must_use]
    pub fn id_at(&self, index: usize) -> Option<Id> {
        let (idx, off) = self.seq.find(index as u64)?;
        Some(self.seq.item(idx).id.offset(off))
    }

    /// Where unit `id` is now, visible or not; `None` if this replica does not hold it.
    #[must_use]
    pub fn locate(&self, id: Id) -> Option<Located> {
        let (idx, off) = self.item_at(id)?;
        let before = self.seq.weight_before(idx).len;
        let visible = !self.seq.item(idx).is_deleted();
        Some(Located {
            index: to_usize(before + if visible { off } else { 0 }),
            visible,
        })
    }

    /// How many visible structural atoms come before visible index `index` (so the unit at
    /// `index` belongs to block `blocks_before(index + 1) - 1`, numbered as in
    /// [`crate::Changes`]).
    #[must_use]
    pub fn blocks_before(&self, index: usize) -> usize {
        match self.seq.find(index as u64) {
            Some((idx, _)) => to_usize(self.seq.weight_before(idx).blocks),
            None => to_usize(self.seq.total().blocks),
        }
    }

    /// The visible unit at `index`.
    #[must_use]
    pub fn unit_at(&self, index: usize) -> Option<Unit> {
        let (idx, off) = self.seq.find(index as u64)?;
        Some(self.unit_of(idx, off))
    }

    fn unit_of(&self, idx: u32, off: u64) -> Unit {
        match self.seq.item(idx).content {
            Content::Text { start, bytes } => {
                let run = &self.text[start as usize..(start + bytes) as usize];
                let ch = run
                    .chars()
                    .nth(to_usize(off))
                    .expect("offset inside the run");
                Unit::Char(ch)
            }
            Content::Atom { kind, .. } => Unit::Atom(kind),
            Content::Gone => unreachable!("a visible unit is not a tombstone"),
        }
    }

    /// The visible units from `index` on, with their ids.
    #[must_use]
    pub fn units_from(&self, index: usize) -> Units<'_> {
        let at = self.seq.find(index as u64).map(|(idx, off)| {
            let byte = match self.seq.item(idx).content {
                Content::Text { start, bytes } => byte_prefix(
                    &self.text[start as usize..(start + bytes) as usize],
                    to_usize(off),
                ),
                _ => 0,
            };
            (idx, off, byte)
        });
        Units { doc: self, at }
    }

    /// Every visible structural atom in document order.
    #[must_use]
    pub fn structure(&self) -> Vec<Structural> {
        let mut out: Vec<Structural> = Vec::new();
        for idx in self.seq.iter() {
            let item = self.seq.item(idx);
            match item.content {
                Content::Atom { kind, .. } if kind.is_structural() => out.push(Structural {
                    id: item.id,
                    kind,
                    inlines: false,
                }),
                Content::Gone => {}
                _ => {
                    if let Some(last) = out.last_mut() {
                        last.inlines = true;
                    }
                }
            }
        }
        out
    }
}

impl Iterator for Units<'_> {
    type Item = (Id, Unit);

    fn next(&mut self) -> Option<(Id, Unit)> {
        let doc = self.doc;
        loop {
            let (idx, off, byte) = self.at?;
            let item = doc.seq.item(idx);
            if item.is_deleted() || off >= u64::from(item.len) {
                self.at = doc.seq.next(idx).map(|n| (n, 0, 0));
                continue;
            }
            let (unit, width) = match item.content {
                Content::Text { start, .. } => {
                    let ch = doc.text[start as usize + byte..]
                        .chars()
                        .next()
                        .expect("a unit inside the run");
                    (Unit::Char(ch), ch.len_utf8())
                }
                Content::Atom { kind, .. } => (Unit::Atom(kind), 0),
                Content::Gone => unreachable!("tombstones are skipped above"),
            };
            self.at = Some((idx, off + 1, byte + width));
            return Some((item.id.offset(off), unit));
        }
    }
}

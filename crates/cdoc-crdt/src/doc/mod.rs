//! The replica: the sequence, the logs that index it by id, marks, attributes, and the buffer
//! of operations that arrived before their dependencies.

mod atoms;
mod delete;
mod error;
mod index;
mod integrate;
mod local;

use crate::attrs::Attrs;
use crate::id::{ClientId, Id, StateVector};
use crate::item::{byte_prefix, Content, Item};
use crate::marks::MarkOp;
use crate::seq::Seq;
use crate::update::ops::DelRange;
use crate::update::Pending;
use std::collections::BTreeMap;

pub(crate) use index::ClockIndex;

pub use error::EditError;

/// What a clock range of a client's history is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entry {
    /// An item (its length is the item's).
    Item(u32),
    /// A mark operation, index into `Doc::marks`.
    Mark(u32),
    /// An attribute write to this key index; one clock.
    Attr(u32),
    /// A delete operation, index into `Doc::delete_ops`; one clock.
    Delete(u32),
    /// Clocks that carry nothing any more (dominated attribute writes, compacted away).
    Skip(u64),
}

/// One client's history: what each clock range is, and the first clock not yet held.
#[derive(Debug, Clone)]
pub(crate) struct ClientLog {
    pub(crate) next: u64,
    pub(crate) entries: ClockIndex,
}

/// A replica of a document.
#[derive(Debug, Clone)]
pub struct Doc {
    pub(crate) client: ClientId,
    /// The greatest Lamport timestamp of any mark held; a new mark takes one more.
    pub(crate) lamport: u64,
    pub(crate) seq: Seq,
    /// Append-only UTF-8 for every text item ever held; items reference byte ranges.
    pub(crate) text: String,
    /// Append-only bytes of every atom payload (codec-encoded model values) ever held.
    pub(crate) atom_buf: Vec<u8>,
    /// Each atom's payload as `(start, len)` in `atom_buf`, by `Content::Atom::index`; a
    /// deleted atom's is empty.
    pub(crate) atoms: Vec<(u32, u32)>,
    pub(crate) logs: BTreeMap<ClientId, ClientLog>,
    pub(crate) marks: Vec<MarkOp>,
    pub(crate) attrs: Attrs,
    /// Every delete operation's ranges, back to back; `delete_ops[k]` is `(first, count)`.
    pub(crate) delete_ranges: Vec<DelRange>,
    pub(crate) delete_ops: Vec<(u32, u32)>,
    /// What garbage collection removed history below: a replica that does not hold all of it
    /// cannot take this replica's updates.
    pub(crate) floor: StateVector,
    pub(crate) pending: Pending,
}

impl Doc {
    /// An empty replica that writes as `client`.
    #[must_use]
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            lamport: 0,
            seq: Seq::new(),
            text: String::new(),
            atom_buf: Vec::new(),
            atoms: Vec::new(),
            logs: BTreeMap::new(),
            marks: Vec::new(),
            attrs: Attrs::new(),
            delete_ranges: Vec::new(),
            delete_ops: Vec::new(),
            floor: StateVector::empty(),
            pending: Pending::new(),
        }
    }

    /// The client this replica writes as.
    #[must_use]
    pub const fn client(&self) -> ClientId {
        self.client
    }

    /// Visible units: characters plus atoms.
    ///
    /// # Panics
    /// Never on a platform whose `usize` holds the document: units are counted in memory.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::try_from(self.seq.total().len).expect("fits the address space")
    }

    /// Whether no unit is visible.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// What this replica holds: its version.
    #[must_use]
    pub fn state_vector(&self) -> StateVector {
        let mut sv = StateVector::empty();
        for (&client, log) in &self.logs {
            sv.set(client, log.next);
        }
        sv
    }

    /// The binary state vector, for a peer to compute a diff against.
    #[must_use]
    pub fn encode_state_vector(&self) -> Vec<u8> {
        self.state_vector().encode()
    }

    /// Whether operations wait for dependencies that have not arrived.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The visible text, with U+FFFC standing in for each atom.
    #[must_use]
    pub fn visible_text(&self) -> String {
        let mut out = String::with_capacity(self.len());
        for idx in self.seq.iter() {
            match self.seq.item(idx).content {
                Content::Text { start, bytes } => {
                    out.push_str(&self.text[start as usize..(start + bytes) as usize]);
                }
                Content::Atom { .. } => out.push('\u{fffc}'),
                Content::Gone => {}
            }
        }
        out
    }

    pub(crate) fn next_clock(&self, client: ClientId) -> u64 {
        self.logs.get(&client).map_or(0, |l| l.next)
    }

    pub(crate) fn has(&self, id: Id) -> bool {
        id.clock < self.next_clock(id.client)
    }

    /// Records `entry` for `len` clocks at `id`, advancing the client's next clock past it.
    pub(crate) fn add_entry(&mut self, id: Id, entry: Entry, len: u64) {
        let log = self.logs.entry(id.client).or_insert_with(|| ClientLog {
            next: 0,
            entries: ClockIndex::new(),
        });
        log.entries.insert(id.clock, entry);
        log.next = log.next.max(id.clock + len);
    }

    /// The entry covering `id`, with its first clock.
    pub(crate) fn entry_at(&self, id: Id) -> Option<(u64, Entry)> {
        let log = self.logs.get(&id.client)?;
        let (start, entry) = log.entries.floor(id.clock)?;
        let len = match entry {
            Entry::Item(i) => u64::from(self.seq.item(i).len),
            Entry::Mark(_) | Entry::Attr(_) | Entry::Delete(_) => 1,
            Entry::Skip(n) => n,
        };
        (id.clock < start + len).then_some((start, entry))
    }

    /// The item holding unit `id`, and the unit's offset in it.
    pub(crate) fn item_at(&self, id: Id) -> Option<(u32, u64)> {
        match self.entry_at(id)? {
            (start, Entry::Item(i)) => Some((i, id.clock - start)),
            _ => None,
        }
    }

    /// Splits an item so its first `at` units stay and the rest become a new item right after
    /// it, as if inserted one by one; returns the new item.
    pub(crate) fn split(&mut self, idx: u32, at: u64) -> u32 {
        let item = self.seq.item(idx);
        let at32 = u32::try_from(at).expect("an offset inside an item fits its length");
        debug_assert!(at32 > 0 && at32 < item.len, "split strictly inside");
        let (left, right) = match item.content {
            Content::Text { start, bytes } => {
                let run = &self.text[start as usize..(start + bytes) as usize];
                let cut = u32::try_from(byte_prefix(run, at32 as usize)).expect("inside a run");
                (
                    Content::Text { start, bytes: cut },
                    Content::Text {
                        start: start + cut,
                        bytes: bytes - cut,
                    },
                )
            }
            other => (other, other),
        };
        let piece = Item::new(
            item.id.offset(at),
            item.len - at32,
            Some(item.id.offset(at - 1)),
            item.origin_right,
            right,
        );
        let id = piece.id;
        self.seq.modify(idx, |i| {
            i.len = at32;
            i.content = left;
        });
        let new = self.seq.insert_after(Some(idx), piece);
        self.add_entry(id, Entry::Item(new), 0);
        new
    }

    /// The item whose last unit is `id`, splitting if needed.
    pub(crate) fn clean_end(&mut self, id: Id) -> Option<u32> {
        let (idx, off) = self.item_at(id)?;
        if off + 1 < u64::from(self.seq.item(idx).len) {
            self.split(idx, off + 1);
        }
        Some(idx)
    }

    /// The item whose first unit is `id`, splitting if needed.
    pub(crate) fn clean_start(&mut self, id: Id) -> Option<u32> {
        let (idx, off) = self.item_at(id)?;
        Some(if off > 0 { self.split(idx, off) } else { idx })
    }
}

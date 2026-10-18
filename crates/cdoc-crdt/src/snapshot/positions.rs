//! Where each unit sits among the units a snapshot writes, both ways: the encoder's id to
//! position (through the replica's logs) and the decoder's position to id (a search over the
//! items read so far).

use crate::bytes::DecodeError;
use crate::doc::Doc;
use crate::id::Id;
use std::cell::OnceCell;
use std::collections::HashMap;

/// Offsets in an item that collection keeps, as `gc::Plan::kept` holds them.
type Kept = HashMap<u32, Vec<(u64, u64)>>;

/// The encoder's side: the position of the first written unit of every item, worked out on
/// the first lookup (a document typed in order needs none).
pub(super) struct Positions<'a> {
    doc: &'a Doc,
    start: OnceCell<Vec<u64>>,
    kept: Option<&'a Kept>,
}

impl<'a> Positions<'a> {
    pub(super) const fn new(doc: &'a Doc, kept: Option<&'a Kept>) -> Self {
        Self {
            doc,
            start: OnceCell::new(),
            kept,
        }
    }

    fn starts(doc: &Doc, kept: Option<&Kept>) -> Vec<u64> {
        let mut start = vec![u64::MAX; doc.seq.item_count()];
        let mut pos = 0;
        for idx in doc.seq.iter() {
            start[idx as usize] = pos;
            pos += match kept.and_then(|k| k.get(&idx)) {
                Some(ranges) => ranges.iter().map(|(a, b)| b - a).sum(),
                None => u64::from(doc.seq.item(idx).len),
            };
        }
        start
    }

    /// The position of unit `id`, when the snapshot writes it.
    pub(super) fn of(&self, id: Id) -> Option<u64> {
        let (idx, off) = self.doc.item_at(id)?;
        let start = self.start.get_or_init(|| Self::starts(self.doc, self.kept));
        let base = start[idx as usize];
        match self.kept.and_then(|k| k.get(&idx)) {
            None => Some(base + off),
            Some(ranges) => {
                let mut before = 0;
                for &(a, b) in ranges {
                    if (a..b).contains(&off) {
                        return Some(base + before + off - a);
                    }
                    before += b - a;
                }
                None
            }
        }
    }
}

/// The decoder's side: the items read so far, in document order.
#[derive(Default)]
pub(super) struct Placed {
    starts: Vec<u64>,
    items: Vec<(Id, u64)>,
    end: u64,
}

impl Placed {
    /// The position the next item starts at.
    pub(super) const fn end(&self) -> u64 {
        self.end
    }

    pub(super) fn push(&mut self, id: Id, len: u64) -> Result<(), DecodeError> {
        self.starts.push(self.end);
        self.items.push((id, len));
        self.end = self
            .end
            .checked_add(len)
            .ok_or(DecodeError::OutOfRange("unit position"))?;
        Ok(())
    }

    /// The unit at position `pos`.
    pub(super) fn id_at(&self, pos: u64) -> Result<Id, DecodeError> {
        let k = self.starts.partition_point(|&s| s <= pos);
        let (id, off) = k
            .checked_sub(1)
            .map(|k| (self.items[k], pos - self.starts[k]))
            .filter(|&((_, len), off)| off < len)
            .map(|((id, _), off)| (id, off))
            .ok_or(DecodeError::Inconsistent(
                "a position outside the snapshot's units",
            ))?;
        Ok(id.offset(off))
    }
}

//! Deletion. A delete is an operation that takes one clock of the deleter, so a state vector
//! says which deletions a replica has seen and a diff ships only the ones a peer lacks. Its
//! effect is state: an item is deleted or it is not, so applying deletes twice, or in any order
//! relative to each other, ends the same way. A deleted item keeps its id and origins and drops
//! its content (a tombstone).

use super::{Doc, Entry};
use crate::id::{ClientId, Id};
use crate::item::Content;
use crate::update::ops::DelRange;

impl Doc {
    /// Deletes one item; whether it was visible.
    pub(crate) fn erase(&mut self, idx: u32) -> bool {
        let item = self.seq.item(idx);
        if item.is_deleted() {
            return false;
        }
        if let Content::Atom { index, .. } = item.content {
            // The bytes stay in the append-only buffer until the next snapshot load.
            self.atoms[index as usize] = (0, 0);
        }
        self.seq.modify(idx, |i| i.content = Content::Gone);
        true
    }

    /// Deletes every held item in `client`'s clocks `start..end`; clocks not held yet are left
    /// to the caller. Newly deleted items are pushed onto `touched`.
    pub(crate) fn delete_range(
        &mut self,
        client: ClientId,
        start: u64,
        end: u64,
        touched: &mut Vec<u32>,
    ) {
        let end = end.min(self.next_clock(client));
        let mut cur = start;
        while cur < end {
            let Some((first, entry)) = self.entry_at(Id::new(client, cur)) else {
                return;
            };
            match entry {
                Entry::Item(mut idx) => {
                    let item = self.seq.item(idx);
                    if item.is_deleted() {
                        // Already a tombstone: nothing to split or erase.
                        cur = item.end();
                        continue;
                    }
                    if cur > first {
                        idx = self.split(idx, cur - first);
                    }
                    let len = u64::from(self.seq.item(idx).len);
                    if cur + len > end {
                        self.split(idx, end - cur);
                    }
                    if self.erase(idx) {
                        touched.push(idx);
                    }
                    cur += len.min(end - cur);
                }
                Entry::Skip(n) => cur = first + n,
                Entry::Mark(_) | Entry::Attr(_) | Entry::Delete(_) => cur = first + 1,
            }
        }
    }

    /// Stores a delete operation's ranges at `id` (one clock); its index.
    pub(crate) fn record_delete(&mut self, id: Id, ranges: &[DelRange]) -> u32 {
        let k = u32::try_from(self.delete_ops.len()).expect("fewer than 2^32 deletes");
        let first = u32::try_from(self.delete_ranges.len()).expect("fewer than 2^32 ranges");
        let count = u32::try_from(ranges.len()).expect("fewer than 2^32 ranges");
        self.delete_ranges.extend_from_slice(ranges);
        self.delete_ops.push((first, count));
        self.add_entry(id, Entry::Delete(k), 1);
        k
    }

    /// The ranges of delete operation `k`.
    pub(crate) fn delete_op(&self, k: u32) -> &[DelRange] {
        let (first, count) = self.delete_ops[k as usize];
        &self.delete_ranges[first as usize..(first + count) as usize]
    }
}

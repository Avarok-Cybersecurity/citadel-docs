//! Deletion. Deletes are state, not operations: an item is deleted or it is not, so applying a
//! delete twice, or in any order relative to other deletes, ends the same way. A deleted item
//! keeps its id and origins and drops its content (a tombstone).

use super::{Doc, Entry};
use crate::id::{ClientId, Id};
use crate::item::Content;

impl Doc {
    /// Deletes one item; whether it was visible.
    pub(crate) fn erase(&mut self, idx: u32) -> bool {
        let item = self.seq.item(idx);
        if item.is_deleted() {
            return false;
        }
        if let Content::Atom { index, .. } = item.content {
            self.atoms[index as usize] = Vec::new();
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
                Entry::Mark(_) | Entry::Attr(_) => cur = first + 1,
            }
        }
    }

    /// Every deleted clock range below `below[client]`, merged, per client in order.
    pub(crate) fn deleted_ranges(
        &self,
        below: impl Fn(ClientId) -> u64,
    ) -> Vec<(ClientId, Vec<(u64, u64)>)> {
        let mut out = Vec::new();
        for (&client, log) in &self.logs {
            let limit = below(client).min(log.next);
            let mut ranges: Vec<(u64, u64)> = Vec::new();
            for (clock, entry) in log.entries.iter().take_while(|&(c, _)| c < limit) {
                let Entry::Item(idx) = entry else { continue };
                let item = self.seq.item(idx);
                if !item.is_deleted() {
                    continue;
                }
                let end = item.end().min(limit);
                match ranges.last_mut() {
                    Some(last) if last.1 == clock => last.1 = end,
                    _ => ranges.push((clock, end)),
                }
            }
            if !ranges.is_empty() {
                out.push((client, ranges));
            }
        }
        out
    }
}

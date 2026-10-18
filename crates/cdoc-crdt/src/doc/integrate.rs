//! Integrating a remote operation whose dependencies are all held.
//!
//! Items are placed with the YATA rule as Yjs implements it. Starting right of the left
//! origin, scan towards the right origin; for each item `o` in the way:
//!
//! - `o` has the same left origin: concurrent inserts at the same place. The lower client id
//!   goes first, so pass `o` if its client is lower; stop if it is higher and shares our right
//!   origin too.
//! - `o`'s left origin lies inside the scanned stretch: `o` belongs to a subtree we are passing,
//!   so pass it, unless its origin is among the items we have not committed to passing.
//! - otherwise `o` was inserted further left than us: stop.
//!
//! Runs are integrated whole. That is exact: every unit of a run after the first has the
//! previous unit as its left origin and shares the run's right origin, so each would follow
//! the first unit's decision.

use super::{Doc, Entry};
use crate::attrs::AttrKey;
use crate::bytes::DecodeError;
use crate::changes::Touched;
use crate::id::Id;
use crate::item::{Content, Item};
use crate::update::ops::{Body, ItemOp, Op};
use std::collections::{BTreeSet, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// A multiplicative hasher for item indices; `SipHash` would dominate the scan.
#[derive(Default)]
struct IndexHasher(u64);

impl Hasher for IndexHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(b)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = u64::from(n).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type IndexSet = HashSet<u32, BuildHasherDefault<IndexHasher>>;

/// Records that attribute `name` changed, copying the name only the first time.
fn touch(attrs: &mut BTreeSet<String>, name: &str) {
    if !attrs.contains(name) {
        attrs.insert(name.to_owned());
    }
}

fn malformed(what: &'static str) -> DecodeError {
    DecodeError::Inconsistent(what)
}

impl Doc {
    /// The first dependency of `op` this replica does not hold.
    pub(crate) fn missing_dep(&self, op: &Op<'_>) -> Option<Id> {
        op.first_missing(|id| self.has(id))
    }

    /// Integrates `op`, whose first unit is `id` (the client's next clock).
    pub(crate) fn integrate(
        &mut self,
        id: Id,
        op: Op<'_>,
        touched: &mut Touched,
    ) -> Result<(), DecodeError> {
        match op {
            Op::Item(item) => self.integrate_item(id, item, touched),
            Op::Mark(mut mark) => {
                for anchor in [mark.start, mark.end] {
                    if let Some(a) = anchor.id().filter(|&a| self.item_at(a).is_none()) {
                        return Err(self.not_an_item(a, "a mark anchor is not an item"));
                    }
                }
                mark.id = id;
                self.lamport = self.lamport.max(mark.lamport);
                let index = u32::try_from(self.marks.len()).map_err(|_| malformed("marks"))?;
                touched.marks.push((mark.start, mark.end));
                self.marks.push(mark);
                self.add_entry(id, Entry::Mark(index), 1);
                Ok(())
            }
            Op::Attr(key, write) => {
                let k = self.attrs.intern(key);
                if self.attrs.write(k, write) {
                    match self.attrs.key(k) {
                        AttrKey::Name(name) => touch(&mut touched.attrs, name),
                        AttrKey::Field(block, _) => touched.blocks.push(*block),
                        AttrKey::Entry(list, _) => touch(&mut touched.attrs, list.attr()),
                    }
                }
                self.add_entry(id, Entry::Attr(k), 1);
                Ok(())
            }
            Op::Skip(n) => {
                self.add_entry(id, Entry::Skip(n), n);
                Ok(())
            }
            Op::Delete(ranges) => {
                for r in &ranges {
                    self.delete_range(r.client, r.start, r.end, &mut touched.items);
                }
                self.record_delete(id, &ranges);
                Ok(())
            }
        }
    }

    /// Why `id`, which an operation names as an item, is not one: collected by garbage
    /// collection (the sender is behind this replica's GC floor), or never an item at all.
    fn not_an_item(&self, id: Id, what: &'static str) -> DecodeError {
        if self.floor.contains(id) {
            DecodeError::Compacted
        } else {
            malformed(what)
        }
    }

    fn integrate_item(
        &mut self,
        id: Id,
        op: ItemOp<'_>,
        touched: &mut Touched,
    ) -> Result<(), DecodeError> {
        let ItemOp {
            len,
            origin_left,
            origin_right,
            body,
        } = op;
        let mut left = match origin_left {
            Some(ol) => Some(
                self.clean_end(ol)
                    .ok_or_else(|| self.not_an_item(ol, "left origin"))?,
            ),
            None => None,
        };
        let right = match origin_right {
            Some(or) => Some(
                self.clean_start(or)
                    .ok_or_else(|| self.not_an_item(or, "right origin"))?,
            ),
            None => None,
        };
        let mut scan = match left {
            Some(l) => self.seq.next(l),
            None => self.seq.first(),
        };
        let mut before_origin = IndexSet::default();
        let mut conflicting = IndexSet::default();
        while let Some(o) = scan {
            if Some(o) == right {
                break;
            }
            let item = self.seq.item(o);
            if self.floor.contains(item.id) {
                // Every peer at or beyond the floor holds this item, so the author of an
                // operation that has to pass it did not: it is behind the floor.
                return Err(DecodeError::Compacted);
            }
            // `Some(true)`: pass `o` and commit to it; `Some(false)`: pass it provisionally;
            // `None`: stop before it. Sets are filled only once `o` is passed, so a scan that
            // stops at the first item (the common case) allocates nothing.
            let pass = if item.origin_left == origin_left {
                if item.id.client < id.client {
                    Some(true)
                } else if item.origin_right == origin_right {
                    None
                } else {
                    Some(false)
                }
            } else if let Some((origin, _)) = item.origin_left.and_then(|ol| self.item_at(ol)) {
                before_origin
                    .contains(&origin)
                    .then(|| !conflicting.contains(&origin))
            } else {
                None
            };
            match pass {
                None => break,
                Some(true) => {
                    before_origin.insert(o);
                    left = Some(o);
                    conflicting.clear();
                }
                Some(false) => {
                    before_origin.insert(o);
                    conflicting.insert(o);
                }
            }
            scan = self.seq.next(o);
        }
        let content = match body {
            Body::Text(text) => {
                if let Some(l) = left {
                    let continues = origin_left == Some(self.seq.item(l).last_id());
                    if continues
                        && self
                            .try_extend(l, id, origin_right, &text)
                            .map_err(|_| malformed("text size"))?
                    {
                        touched.items.push(l);
                        return Ok(());
                    }
                }
                let (start, bytes) = self.push_text(&text).map_err(|_| malformed("text size"))?;
                Content::Text { start, bytes }
            }
            Body::Atom(kind, payload) => {
                let index = self.push_atom(&payload).ok_or(malformed("atoms"))?;
                Content::Atom { index, kind }
            }
            Body::Gone => Content::Gone,
        };
        let idx = self
            .seq
            .insert_after(left, Item::new(id, len, origin_left, origin_right, content));
        self.add_entry(id, Entry::Item(idx), u64::from(len));
        touched.items.push(idx);
        Ok(())
    }
}

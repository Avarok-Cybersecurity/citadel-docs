//! The snapshot's operations other than items: marks, surviving attribute writes and delete
//! operations, each client's in clock order, clients ascending.
//!
//! ```text
//! ops      := n:varint (head op_body | switch)*      (n counts operations, not switches)
//! head     := varint gap << 3 | tag   (tag 0 mark, 1 attribute, 2 delete, 4 built-in styles;
//!             the clock is the gap from the end of the client's previous operation, or from
//!             0; a gap of LONG is followed by the real gap as a varint)
//! switch   := varint step << 3 | 3     (move `step` > 0 clients on; the clock restarts at 0)
//! op_body  := mark | attr | delete | builtin   (mark and attr as in an update)
//! builtin  := deps:state_vector rank:varint    (Word's built-in styles, `builtin.rs`)
//! delete   := k << 1 | explicit:varint range{k}
//! range    := explicit = 0: start:zvar len:varint   the position of the range's first unit,
//!             as a difference from the previous range's position plus its length (or 0)
//!             explicit = 1: form:u8 (0 as above, 1 client_index:varint clock:varint) len
//! ```
//!
//! A range is a run of clocks; only its first unit is located by position, the rest follow in
//! clock order wherever they sit.

use super::builtin;
use super::items::{client_index, write_explicit};
use super::positions::Positions;
use crate::attrs::AttrWrite;
use crate::bytes::Writer;
use crate::doc::{Doc, Entry};
use crate::gc::Plan;
use crate::id::{ClientId, Id, StateVector};
use crate::update::ops::DelRange;
use crate::update::wire::{write_attr, write_mark};

pub(super) const MARK: u64 = 0;
pub(super) const ATTR: u64 = 1;
pub(super) const DELETE: u64 = 2;
pub(super) const SWITCH: u64 = 3;
pub(super) const BUILTIN: u64 = 4;
pub(super) const TAG_BITS: u32 = 3;
/// A gap too large for the head: the real one follows.
pub(super) const LONG: u64 = u64::MAX >> TAG_BITS;

fn head(w: &mut Writer, gap: u64, tag: u64) {
    w.uvar(gap.min(LONG) << TAG_BITS | tag);
    if gap >= LONG {
        w.uvar(gap);
    }
}

fn write_ranges(
    w: &mut Writer,
    clients: &[ClientId],
    positions: &Positions<'_>,
    ranges: &[DelRange],
) {
    let located: Vec<Option<u64>> = ranges
        .iter()
        .map(|r| positions.of(Id::new(r.client, r.start)))
        .collect();
    let explicit = located.iter().any(Option::is_none);
    w.uvar((ranges.len() as u64) << 1 | u64::from(explicit));
    let mut cursor = 0u64;
    for (r, at) in ranges.iter().zip(located) {
        if let Some(pos) = at {
            if explicit {
                w.u8(0);
            }
            // The wrapping difference reinterpreted as signed: the zigzag's input.
            #[allow(clippy::cast_possible_wrap)]
            w.zvar(pos.wrapping_sub(cursor) as i64);
            cursor = pos.wrapping_add(r.end - r.start);
        } else {
            w.u8(1);
            write_explicit(w, clients, Id::new(r.client, r.start));
        }
        w.uvar(r.end - r.start);
    }
}

/// What one operation of the section writes.
enum Body<'d> {
    Mark(u32),
    Attr(u32, &'d AttrWrite),
    Delete(u32),
    Builtin(StateVector, u64),
}

/// The operations section, and how many operations it holds.
pub(super) fn write_ops(
    doc: &Doc,
    gc: Option<&Plan>,
    clients: &[ClientId],
    positions: &Positions<'_>,
) -> (usize, Vec<u8>) {
    let mut w = Writer::new();
    let mut n = 0usize;
    let mut at_client = 0usize;
    for (&client, log) in &doc.logs {
        let ci = client_index(clients, client);
        let mut next = 0u64;
        for (clock, entry) in log.entries.iter() {
            if clock < next {
                continue; // inside a built-in run already written
            }
            let id = Id::new(client, clock);
            let body = match entry {
                Entry::Mark(m) => Body::Mark(m),
                Entry::Attr(k) => match builtin::run_at(doc, client, clock, k) {
                    Some((deps, rank)) => Body::Builtin(deps, rank),
                    None => match doc.attrs.surviving(k, id) {
                        Some(write) => Body::Attr(k, write),
                        None => continue,
                    },
                },
                Entry::Delete(_) if gc.is_some_and(|plan| plan.drops_delete(id)) => continue,
                Entry::Delete(k) => Body::Delete(k),
                Entry::Item(_) | Entry::Skip(_) => continue,
            };
            if ci != at_client {
                w.uvar(((ci - at_client) as u64) << TAG_BITS | SWITCH);
                at_client = ci;
            }
            let (tag, len) = match body {
                Body::Mark(_) => (MARK, 1),
                Body::Attr(..) => (ATTR, 1),
                Body::Delete(_) => (DELETE, 1),
                Body::Builtin(..) => (BUILTIN, builtin::len()),
            };
            head(&mut w, clock - next, tag);
            next = clock + len;
            match body {
                Body::Mark(m) => write_mark(&mut w, &doc.marks[m as usize]),
                Body::Attr(k, write) => write_attr(&mut w, doc.attrs.key(k), write),
                Body::Delete(k) => write_ranges(&mut w, clients, positions, doc.delete_op(k)),
                Body::Builtin(deps, rank) => {
                    deps.write(&mut w);
                    w.uvar(rank);
                }
            }
            n += 1;
        }
    }
    (n, w.finish())
}

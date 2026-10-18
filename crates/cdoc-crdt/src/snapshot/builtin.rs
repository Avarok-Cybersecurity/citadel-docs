//! Word's built-in styles in a snapshot as one operation. A document made with `from_model`
//! from a model holding `StyleSheet::word_builtin()`'s styles writes them as consecutive
//! element writes of one client (`bridge/entries.rs`): write `i` is at clock `c + i`, adds
//! built-in style `i` with rank `r + i`, and saw the state vector `d` with the writer at
//! `c + i`. When a snapshot holds exactly that run, every write surviving, it writes the
//! marker operation `d r` in its place and the decoder makes the same writes again. Anything
//! else, one style edited or one write dominated, is written write by write: the marker is
//! explicit, never inferred from what a reader lacks.

use crate::attrs::{AttrKey, AttrWrite, List};
use crate::bridge::entry_value;
use crate::bytes::Reader;
use crate::codec::to_bytes;
use crate::doc::{Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use cdoc_model::StyleSheet;
use std::sync::OnceLock;

/// Each built-in style's encoded id (the element key) and encoded style, in sheet order.
fn table() -> &'static [(Vec<u8>, Vec<u8>)] {
    static TABLE: OnceLock<Vec<(Vec<u8>, Vec<u8>)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        StyleSheet::word_builtin()
            .styles
            .iter()
            .map(|s| {
                let key = to_bytes(&s.id).expect("a style id encodes");
                let style = to_bytes(s).expect("a style holds no floats");
                (key, style)
            })
            .collect()
    })
}

/// How many clocks the marker covers.
pub(super) fn len() -> u64 {
    table().len() as u64
}

/// The `i`-th write of the run that starts at `at`, rank `rank`, having seen `deps`.
fn write(at: Id, deps: &StateVector, rank: u64, i: usize) -> (AttrKey, AttrWrite) {
    let (key, style) = &table()[i];
    let id = at.offset(i as u64);
    let mut seen = deps.clone();
    seen.set(at.client, id.clock);
    let value = entry_value((rank + i as u64, id), style);
    let write = AttrWrite {
        id,
        deps: seen,
        value,
    };
    (AttrKey::Entry(List::Styles, key.clone()), write)
}

/// Every write of the run at `at`, to integrate in clock order.
pub(super) fn writes(
    at: Id,
    deps: &StateVector,
    rank: u64,
) -> impl Iterator<Item = (AttrKey, AttrWrite)> + '_ {
    (0..table().len()).map(move |i| write(at, deps, rank, i))
}

/// The marker's `(deps, rank)` when `client`'s operations from `clock` on are the built-in
/// run, every write surviving. Compares in place: nothing is built for a document without it.
pub(super) fn run_at(
    doc: &Doc,
    client: ClientId,
    clock: u64,
    k: u32,
) -> Option<(StateVector, u64)> {
    // Most attribute writes are not the run's first: one key comparison turns them away.
    match doc.attrs.key(k) {
        AttrKey::Entry(List::Styles, key) if *key == table()[0].0 => {}
        _ => return None,
    }
    let at = Id::new(client, clock);
    let first = held(doc, at, 0)?;
    let rank = Reader::new(&first.value).uvar().ok()?;
    if first.deps.get(client) != clock {
        return None;
    }
    for i in 0..table().len() {
        let id = at.offset(i as u64);
        let write = held(doc, id, i)?;
        let mut r = Reader::new(&write.value);
        let born = (r.uvar().ok()?, r.uvar().ok()?, r.uvar().ok()?);
        let tail = &write.value[r.position()..];
        if born != (rank + i as u64, client.0, id.clock)
            || tail != table()[i].1.as_slice()
            || !seen_alike(&write.deps, &first.deps, client, id.clock)
        {
            return None;
        }
    }
    Some((first.deps.clone(), rank))
}

/// The surviving write at `id` when it is the element write of built-in style `i`.
fn held(doc: &Doc, id: Id, i: usize) -> Option<&AttrWrite> {
    let (start, Entry::Attr(k)) = doc.entry_at(id)? else {
        return None;
    };
    match doc.attrs.key(k) {
        AttrKey::Entry(List::Styles, key) if start == id.clock && *key == table()[i].0 => {
            doc.attrs.surviving(k, id)
        }
        _ => None,
    }
}

/// Whether `a` is `b` with `own` at `next`.
fn seen_alike(a: &StateVector, b: &StateVector, own: ClientId, next: u64) -> bool {
    a.get(own) == next
        && a.iter().all(|(c, n)| c == own || b.get(c) == n)
        && b.iter().all(|(c, n)| c == own || a.get(c) == n)
}

//! Client 0 ([`ClientId::BLANK`]) writes only the blank document's base: every replica opened
//! blank holds the same operations at the same clocks, and no replica writes as it. An
//! incoming operation claiming client 0 is accepted only when it is one of those operations
//! (or the part of one a diff trims to), or the deleted form of a run of them, so a peer cannot
//! plant another base under the id every blank shares. A loaded snapshot is held to the same
//! rule.

use super::decode::decode;
use super::ops::{Body, ItemOp, Op};
use crate::bridge::blank;
use crate::doc::{Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use crate::item::byte_prefix;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The blank document's operations, by first clock.
fn base() -> &'static BTreeMap<u64, Op<'static>> {
    static BASE: OnceLock<BTreeMap<u64, Op<'static>>> = OnceLock::new();
    BASE.get_or_init(|| {
        let doc = blank(ClientId(1)).expect("the blank document builds");
        let bytes = doc.encode_diff(&StateVector::empty());
        let decoded = decode(&bytes).expect("a replica's own diff decodes");
        decoded
            .clients
            .into_iter()
            .filter(|(client, _)| *client == ClientId::BLANK)
            .flat_map(|(_, ops)| ops)
            .map(|(clock, op)| (clock, op.into_owned()))
            .collect()
    })
}

/// Why an update or a snapshot claiming client 0 for anything but the base is refused.
pub(crate) const NOT_BASE: &str = "client 0 writes only the blank document";

/// Whether `op` at `clock` is the blank document's operation there.
pub(crate) fn is_base(clock: u64, op: &Op<'_>) -> bool {
    match op {
        Op::Item(item) => is_base_item(clock, item),
        other => base().get(&clock) == Some(other),
    }
}

/// Whether the items and the clock `doc` holds for client 0 are the base's, for a loaded
/// snapshot (its operations are checked with [`is_base`] as they load). A gap is a base item
/// collected after it was deleted.
pub(crate) fn holds_only_base(doc: &Doc) -> bool {
    let Some(log) = doc.logs.get(&ClientId::BLANK) else {
        return true;
    };
    let end = base()
        .last_key_value()
        .map_or(0, |(&clock, op)| clock + op.len());
    log.next <= end
        && log.entries.iter().all(|(clock, entry)| match entry {
            Entry::Item(idx) => is_base_item(clock, &doc.item_op(idx, 0)),
            Entry::Mark(_) | Entry::Attr(_) | Entry::Delete(_) | Entry::Skip(_) => true,
        })
}

/// An item is the base's units `clock..clock + len` with their origins: inside one base item
/// with its content, or, deleted, over a run of them that a diff sends as one.
fn is_base_item(clock: u64, item: &ItemOp<'_>) -> bool {
    let gone = matches!(item.body, Body::Gone);
    let end = clock + u64::from(item.len);
    let mut at = clock;
    while at < end {
        let Some(held) = piece(at, end) else {
            return false;
        };
        let left = if at == clock {
            item.origin_left
        } else {
            Some(Id::new(ClientId::BLANK, at - 1))
        };
        let whole = at == clock && u64::from(held.len) == u64::from(item.len);
        if held.origin_left != left
            || held.origin_right != item.origin_right
            || !(gone || (whole && held.body == item.body))
        {
            return false;
        }
        at += u64::from(held.len);
    }
    true
}

/// The base item holding unit `at`, cut to `at..end` (or to its own end, if sooner).
fn piece(at: u64, end: u64) -> Option<ItemOp<'static>> {
    let (&start, held) = base().range(..=at).next_back()?;
    let Op::Item(held) = held else {
        return None;
    };
    let held_end = start + u64::from(held.len);
    if at >= held_end {
        return None;
    }
    let mut cut = if at == start {
        held.clone()
    } else {
        held.clone()
            .trim(Id::new(ClientId::BLANK, start), at - start)
    };
    let keep = u32::try_from(end.min(held_end) - at).expect("inside the item");
    if let Body::Text(text) = &mut cut.body {
        let bytes = byte_prefix(text, keep as usize);
        text.to_mut().truncate(bytes);
    }
    cut.len = keep;
    Some(cut)
}

#[cfg(test)]
mod tests {
    use super::{base, holds_only_base, is_base, Body, ClientId, Doc, Entry, Id, ItemOp, Op};
    use crate::doc::{ClientLog, ClockIndex};

    /// The base's first run of two adjacent items, as one deleted item.
    fn deleted_run() -> (u64, ItemOp<'static>) {
        let items: Vec<(u64, &ItemOp<'static>)> = base()
            .iter()
            .filter_map(|(&clock, op)| match op {
                Op::Item(item) => Some((clock, item)),
                _ => None,
            })
            .collect();
        let (clock, first, second) = items
            .windows(2)
            .find_map(|w| {
                let ((c, a), (d, b)) = (w[0], w[1]);
                let chained = c + u64::from(a.len) == d
                    && b.origin_left == Some(Id::new(ClientId::BLANK, d - 1))
                    && b.origin_right == a.origin_right;
                chained.then_some((c, a, b))
            })
            .expect("the base has two chained items");
        let run = ItemOp {
            len: first.len + second.len,
            origin_left: first.origin_left,
            origin_right: first.origin_right,
            body: Body::Gone,
        };
        (clock, run)
    }

    #[test]
    fn a_deleted_run_of_base_items_is_the_base() {
        let (clock, run) = deleted_run();
        assert!(is_base(clock, &Op::Item(run)));
    }

    #[test]
    fn a_deleted_run_with_other_origins_is_not() {
        let (clock, run) = deleted_run();
        let elsewhere = Some(Id::new(ClientId(9), 0));
        for forged in [
            ItemOp {
                origin_right: elsewhere,
                ..run.clone()
            },
            ItemOp {
                origin_left: elsewhere,
                ..run.clone()
            },
            ItemOp {
                len: run.len + 1_000_000,
                ..run
            },
        ] {
            assert!(!is_base(clock, &Op::Item(forged)));
        }
    }

    /// A replica whose only entry for client 0 is a gap of `len` from clock 0.
    fn collected(len: u64) -> Doc {
        let mut doc = Doc::new(ClientId(5));
        let entries = ClockIndex::from_sorted(&[(0, Entry::Skip(len))]);
        doc.logs
            .insert(ClientId::BLANK, ClientLog { next: len, entries });
        doc
    }

    #[test]
    fn a_collected_base_is_the_base_and_a_clock_past_it_is_not() {
        let end = base()
            .last_key_value()
            .map(|(&clock, op)| clock + op.len())
            .expect("the base is not empty");
        assert!(holds_only_base(&collected(end)));
        assert!(!holds_only_base(&collected(end + 1)));
    }
}

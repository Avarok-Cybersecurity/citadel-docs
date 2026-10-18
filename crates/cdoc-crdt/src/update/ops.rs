//! Operations as they travel: decoded from an update, borrowing its bytes until integrated, or
//! owned while they wait in the pending buffer.

use crate::attrs::{AttrKey, AttrWrite};
use crate::id::{ClientId, Id};
use crate::item::{byte_prefix, AtomKind};
use crate::marks::MarkOp;
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Body<'a> {
    Text(Cow<'a, str>),
    Atom(AtomKind, Cow<'a, [u8]>),
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemOp<'a> {
    pub(crate) len: u32,
    pub(crate) origin_left: Option<Id>,
    pub(crate) origin_right: Option<Id>,
    pub(crate) body: Body<'a>,
}

/// The units `start..end` of `client`'s history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DelRange {
    pub(crate) client: ClientId,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Sorts `ranges` and merges those that overlap or touch: the canonical form of a delete.
pub(crate) fn canonical(mut ranges: Vec<DelRange>) -> Vec<DelRange> {
    ranges.sort_unstable();
    let mut out: Vec<DelRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(p) if p.client == r.client && p.end >= r.start => p.end = p.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Op<'a> {
    Item(ItemOp<'a>),
    Mark(MarkOp),
    Attr(AttrKey, AttrWrite),
    Skip(u64),
    /// Deletes these units; one clock of the deleter, so a state vector says who has seen it.
    Delete(Vec<DelRange>),
}

impl Op<'_> {
    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Item(i) => u64::from(i.len),
            Self::Mark(_) | Self::Attr(..) | Self::Delete(_) => 1,
            Self::Skip(n) => *n,
        }
    }

    /// The first unit this operation needs that `has` says is not held.
    pub(crate) fn first_missing(&self, has: impl Fn(Id) -> bool) -> Option<Id> {
        let missing = |id: Option<Id>| id.filter(|&i| !has(i));
        match self {
            Self::Item(i) => missing(i.origin_left).or_else(|| missing(i.origin_right)),
            Self::Mark(m) => missing(m.start.id()).or_else(|| missing(m.end.id())),
            Self::Attr(key, w) => {
                let block = match key {
                    AttrKey::Field(id, _) => Some(*id),
                    AttrKey::Name(_) | AttrKey::Entry(..) => None,
                };
                missing(block).or_else(|| {
                    w.deps
                        .iter()
                        .map(|(c, n)| Id::new(c, n - 1))
                        .find(|&i| !has(i))
                })
            }
            Self::Skip(_) => None,
            // Holding a client's last deleted unit means holding the whole range.
            Self::Delete(ranges) => ranges
                .iter()
                .map(|r| Id::new(r.client, r.end - 1))
                .find(|&i| !has(i)),
        }
    }

    /// Drops the first `by` units (already held), as if they had been sent separately.
    pub(crate) fn trim(self, own: Id, by: u64) -> Self {
        match self {
            Self::Item(item) => {
                let by32 = u32::try_from(by).expect("trimmed inside the item");
                let body = match item.body {
                    Body::Text(text) => {
                        let cut = byte_prefix(&text, by32 as usize);
                        Body::Text(match text {
                            Cow::Borrowed(t) => Cow::Borrowed(&t[cut..]),
                            Cow::Owned(t) => Cow::Owned(t[cut..].to_owned()),
                        })
                    }
                    other => other,
                };
                Self::Item(ItemOp {
                    len: item.len - by32,
                    origin_left: Some(own.offset(by - 1)),
                    origin_right: item.origin_right,
                    body,
                })
            }
            Self::Skip(n) => Self::Skip(n - by),
            other => other,
        }
    }

    pub(crate) fn into_owned(self) -> Op<'static> {
        match self {
            Self::Item(i) => Op::Item(ItemOp {
                len: i.len,
                origin_left: i.origin_left,
                origin_right: i.origin_right,
                body: match i.body {
                    Body::Text(t) => Body::Text(Cow::Owned(t.into_owned())),
                    Body::Atom(k, p) => Body::Atom(k, Cow::Owned(p.into_owned())),
                    Body::Gone => Body::Gone,
                },
            }),
            Self::Mark(m) => Op::Mark(m),
            Self::Attr(k, w) => Op::Attr(k, w),
            Self::Skip(n) => Op::Skip(n),
            Self::Delete(r) => Op::Delete(r),
        }
    }
}

/// Operations that arrived before their dependencies.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) ops: BTreeMap<ClientId, BTreeMap<u64, Op<'static>>>,
}

impl Pending {
    pub(crate) const fn new() -> Self {
        Self {
            ops: BTreeMap::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

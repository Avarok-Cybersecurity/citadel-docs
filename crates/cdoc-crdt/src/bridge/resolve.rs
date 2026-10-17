//! Which marks cover which units.
//!
//! Every unit (tombstones included) has a position `u` in the full sequence; the gaps around it
//! are numbered `2u` (`Before`) and `2u + 1` (`After`). A mark from gap `s` to gap `e` covers
//! unit `u` exactly when `s <= 2u` and `2u + 1 <= e`, so it switches on at unit `ceil(s / 2)`
//! and off at unit `ceil(e / 2)`. Positions never change once assigned, which is why ranges
//! anchored this way commute with every concurrent insertion.

use super::run_props;
use super::BridgeError;
use crate::codec::from_bytes;
use crate::doc::Doc;
use crate::marks::{Anchor, MarkKey};
use cdoc_model::{CommentId, Marks, RunProperties};
use std::collections::BTreeMap;

struct Switch {
    unit: u64,
    on: bool,
    mark: u32,
}

pub(crate) struct Resolver<'d> {
    doc: &'d Doc,
    switches: Vec<Switch>,
    next: usize,
    single: BTreeMap<(MarkKey, u64, u64), u32>,
    comments: BTreeMap<(u64, u64), u32>,
    cache: Option<(RunProperties, Marks)>,
}

impl<'d> Resolver<'d> {
    /// `positions[idx]` is the full position of item `idx`'s first unit.
    pub(crate) fn new(doc: &'d Doc, positions: &[u64]) -> Self {
        let gap = |anchor: Anchor| -> u64 {
            match anchor {
                Anchor::Start => 0,
                Anchor::End => u64::MAX,
                Anchor::Before(id) | Anchor::After(id) => {
                    let (idx, off) = doc.item_at(id).expect("anchors are integrated items");
                    let unit = positions[idx as usize] + off;
                    2 * unit + u64::from(matches!(anchor, Anchor::After(_)))
                }
            }
        };
        let mut switches = Vec::with_capacity(doc.marks.len() * 2);
        for (i, m) in doc.marks.iter().enumerate() {
            let on = gap(m.start).div_ceil(2);
            let off = gap(m.end).div_ceil(2);
            if on < off {
                let mark = u32::try_from(i).expect("fewer than 2^32 marks");
                switches.push(Switch {
                    unit: on,
                    on: true,
                    mark,
                });
                switches.push(Switch {
                    unit: off,
                    on: false,
                    mark,
                });
            }
        }
        switches.sort_by_key(|s| s.unit);
        Self {
            doc,
            switches,
            next: 0,
            single: BTreeMap::new(),
            comments: BTreeMap::new(),
            cache: None,
        }
    }

    /// Applies every switch at or before unit `u`.
    pub(crate) fn seek(&mut self, u: u64) {
        while let Some(s) = self.switches.get(self.next) {
            if s.unit > u {
                break;
            }
            let m = &self.doc.marks[s.mark as usize];
            let (lamport, client) = m.stamp();
            match &m.key {
                MarkKey::Comment(_) => {
                    if s.on {
                        self.comments.insert((lamport, client), s.mark);
                    } else {
                        self.comments.remove(&(lamport, client));
                    }
                }
                key => {
                    if s.on {
                        self.single.insert((key.clone(), lamport, client), s.mark);
                    } else {
                        self.single.remove(&(key.clone(), lamport, client));
                    }
                }
            }
            self.cache = None;
            self.next += 1;
        }
    }

    /// The first unit after the current one at which coverage may change.
    pub(crate) fn next_change(&self) -> u64 {
        self.switches.get(self.next).map_or(u64::MAX, |s| s.unit)
    }

    fn winner(&self, key: &MarkKey) -> Option<&'d [u8]> {
        let lo = (key.clone(), 0, 0);
        let hi = (key.clone(), u64::MAX, u64::MAX);
        let (_, &m) = self.single.range(lo..=hi).next_back()?;
        self.doc.marks[m as usize].value.as_deref()
    }

    /// Character properties and marks of the units the resolver is at.
    pub(crate) fn current(&mut self) -> Result<(RunProperties, Marks), BridgeError> {
        if let Some(c) = &self.cache {
            return Ok(c.clone());
        }
        let mut run = RunProperties::none();
        for key in crate::marks::RunKey::ALL {
            if let Some(bytes) = self.winner(&MarkKey::Run(key)) {
                run_props::set(&mut run, key, bytes)?;
            }
        }
        let mut comments: Vec<CommentId> = Vec::new();
        for &m in self.comments.values() {
            let op = &self.doc.marks[m as usize];
            let MarkKey::Comment(id) = &op.key else {
                continue;
            };
            if op.value.is_some() {
                comments.push(CommentId::new(id.clone()));
            } else {
                comments.retain(|c| c.as_str() != id);
            }
        }
        let marks = Marks {
            link: self.winner(&MarkKey::Link).map(from_bytes).transpose()?,
            comments,
            revision: self
                .winner(&MarkKey::Revision)
                .map(from_bytes)
                .transpose()?,
        };
        self.cache = Some((run.clone(), marks.clone()));
        Ok((run, marks))
    }
}

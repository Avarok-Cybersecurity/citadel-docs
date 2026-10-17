//! Loading a snapshot: one pass, no integration. Items arrive in document order and are
//! appended; the text section becomes the text buffer with one copy, and each text item is a
//! byte range of it.

use crate::bytes::{DecodeError, Reader};
use crate::changes::Touched;
use crate::doc::{ClientLog, ClockIndex, Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use crate::item::{Content, Item};
use crate::seq::Seq;
use crate::update::ops::Body;
use crate::update::read_op;
use crate::update::wire::{read_header, read_item, tag_of, ATTR, KIND_SNAPSHOT, MARK};
use std::borrow::Cow;

fn bad(what: &'static str) -> DecodeError {
    DecodeError::Inconsistent(what)
}

impl Doc {
    /// Loads a snapshot into a replica that will write as `client`.
    pub fn decode_snapshot(bytes: &[u8], client: ClientId) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        read_header(&mut r, KIND_SNAPSHOT)?;
        let sv = StateVector::read(&mut r)?;
        let clients: Vec<(ClientId, u64)> = sv.iter().collect();
        let section = r.str()?;
        let mut doc = Self::new(client);
        section.clone_into(&mut doc.text);
        let mut entries: Vec<Vec<(u64, Entry)>> = vec![Vec::new(); clients.len()];
        let mut offset = 0usize;
        for _ in 0..r.usize("item count")? {
            let ci = r.usize("client index")?;
            let &(owner, next) = clients.get(ci).ok_or(bad("client index"))?;
            let id = Id::new(owner, r.uvar()?);
            let head = r.u8()?;
            let mut start = 0;
            let op = read_item(&mut r, head, id, |r| {
                let n = usize::try_from(r.uvar()?).map_err(|_| bad("text section"))?;
                start = offset;
                let end = offset.checked_add(n).ok_or(bad("text section"))?;
                let text = section.get(offset..end).ok_or(bad("text section"))?;
                offset = end;
                Ok(Cow::Borrowed(text))
            })?;
            if id
                .clock
                .checked_add(u64::from(op.len))
                .is_none_or(|end| end > next)
            {
                return Err(bad("an item past its client's clock"));
            }
            for origin in op.origin_left.into_iter().chain(op.origin_right) {
                if !sv.contains(origin) {
                    return Err(bad("an origin outside the snapshot"));
                }
            }
            let content = match op.body {
                Body::Text(text) => Content::Text {
                    start: u32::try_from(start).map_err(|_| bad("text size"))?,
                    bytes: u32::try_from(text.len()).map_err(|_| bad("text size"))?,
                },
                Body::Atom(kind, payload) => {
                    let index = u32::try_from(doc.atoms.len()).map_err(|_| bad("atoms"))?;
                    doc.atoms.push(payload.into_owned());
                    Content::Atom { index, kind }
                }
                Body::Gone => Content::Gone,
            };
            let item = Item::new(id, op.len, op.origin_left, op.origin_right, content);
            let idx = doc.seq.push(item);
            entries[ci].push((id.clock, Entry::Item(idx)));
        }
        if offset != section.len() {
            return Err(bad("unused text"));
        }
        doc.logs = clients
            .iter()
            .zip(entries)
            .map(|(&(client, next), mut list)| {
                list.sort_unstable_by_key(|(clock, _)| *clock);
                let entries = ClockIndex::from_sorted(&list);
                (client, ClientLog { next, entries })
            })
            .collect();
        let mut ignored = Touched::default();
        for _ in 0..r.usize("operation count")? {
            let ci = r.usize("client index")?;
            let &(owner, _) = clients.get(ci).ok_or(bad("client index"))?;
            let id = Id::new(owner, r.uvar()?);
            let head = r.u8()?;
            if !matches!(tag_of(head), MARK | ATTR) {
                return Err(DecodeError::BadTag {
                    what: "snapshot operation",
                    tag: head,
                });
            }
            let op = read_op(&mut r, head, id)?;
            if op.first_missing(|d| sv.contains(d)).is_some() {
                return Err(bad("a dependency outside the snapshot"));
            }
            if doc.entry_at(id).is_some() {
                return Err(bad("overlapping operations"));
            }
            doc.integrate(id, op, &mut ignored)?;
        }
        r.finish()?;
        for log in doc.logs.values_mut() {
            tile(&doc.seq, log)?;
        }
        doc.lamport = doc.marks.iter().map(|m| m.lamport).max().unwrap_or(0);
        Ok(doc)
    }
}

/// Checks a client's entries do not overlap and fills the gaps between them with skips.
fn tile(seq: &Seq, log: &mut ClientLog) -> Result<(), DecodeError> {
    let mut gaps = Vec::new();
    let mut cur = 0u64;
    for (clock, entry) in log.entries.iter() {
        if clock < cur {
            return Err(bad("overlapping operations"));
        }
        if clock > cur {
            gaps.push((cur, clock - cur));
        }
        cur = clock
            + match entry {
                Entry::Item(i) => u64::from(seq.item(i).len),
                Entry::Mark(_) | Entry::Attr(_) => 1,
                Entry::Skip(n) => n,
            };
    }
    if cur > log.next {
        return Err(bad("operations past the state vector"));
    }
    if cur < log.next {
        gaps.push((cur, log.next - cur));
    }
    for (clock, len) in gaps {
        log.entries.insert(clock, Entry::Skip(len));
    }
    Ok(())
}

//! Loading a snapshot: one pass, no integration. Items arrive in document order and are
//! appended; the text section becomes the text buffer with one copy, and each text item is a
//! byte range of it.

use super::builtin;
use super::items::read_origins;
use super::ops_read::{OpReader, Read};
use super::positions::Placed;
use crate::bytes::{DecodeError, Reader};
use crate::changes::Touched;
use crate::doc::{ClientLog, ClockIndex, Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use crate::item::{Content, Item};
use crate::seq::Seq;
use crate::update::ops::{Body, Op};
use crate::update::wire::{read_body, read_header, tag_of, KIND_SNAPSHOT};
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
        let floor = StateVector::read(&mut r)?;
        if !sv.covers(&floor) {
            return Err(bad("a GC floor past the state vector"));
        }
        let clients: Vec<(ClientId, u64)> = sv.iter().collect();
        let section = r.str()?;
        // One scan here spares a character count per text item.
        let ascii = section.is_ascii();
        let mut doc = Self::new(client);
        doc.floor = floor;
        section.clone_into(&mut doc.text);
        let mut entries: Vec<Vec<(u64, Entry)>> = vec![Vec::new(); clients.len()];
        let mut offset = 0usize;
        let mut placed = Placed::default();
        let mut cursors = vec![0u64; clients.len()];
        let mut rights: Vec<(u32, u64)> = Vec::new();
        let items = r.usize("item count")?;
        doc.seq.reserve(items.min(r.remaining()));
        for _ in 0..items {
            let ci = r.usize("client index")?;
            let &(owner, next) = clients.get(ci).ok_or(bad("client index"))?;
            // The zigzag's output reinterpreted as the wrapping difference it encodes.
            #[allow(clippy::cast_sign_loss)]
            let id = Id::new(owner, cursors[ci].wrapping_add(r.zvar()? as u64));
            let head = r.u8()?;
            let origins = read_origins(&mut r, head, id, &placed, &clients, &sv)?;
            let mut start = 0;
            let (len, body) = read_body(&mut r, tag_of(head), |r| {
                let n = usize::try_from(r.uvar()?).map_err(|_| bad("text section"))?;
                start = offset;
                let end = offset.checked_add(n).ok_or(bad("text section"))?;
                let text = section.get(offset..end).ok_or(bad("text section"))?;
                offset = end;
                Ok((Cow::Borrowed(text), ascii))
            })?;
            let end = id
                .clock
                .checked_add(u64::from(len))
                .filter(|&end| end <= next)
                .ok_or(bad("an item past its client's clock"))?;
            cursors[ci] = end;
            let content = match body {
                Body::Text(text) => Content::Text {
                    start: u32::try_from(start).map_err(|_| bad("text size"))?,
                    bytes: u32::try_from(text.len()).map_err(|_| bad("text size"))?,
                },
                Body::Atom(kind, payload) => {
                    let index = doc.push_atom(&payload).ok_or(bad("atoms"))?;
                    Content::Atom { index, kind }
                }
                Body::Gone => Content::Gone,
            };
            let idx = doc
                .seq
                .push(Item::new(id, len, origins.left, origins.right, content));
            placed.push(id, u64::from(len))?;
            if let Some(d) = origins.ahead {
                let at = placed
                    .end()
                    .checked_add(d)
                    .ok_or(bad("an origin outside the snapshot"))?;
                rights.push((idx, at));
            }
            entries[ci].push((id.clock, Entry::Item(idx)));
        }
        for (idx, at) in rights {
            let origin = placed.id_at(at)?;
            doc.seq.modify(idx, |item| item.origin_right = Some(origin));
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
        doc.load_ops(&mut r, &sv, &clients, &placed)?;
        r.finish()?;
        for log in doc.logs.values_mut() {
            tile(&doc.seq, log)?;
        }
        doc.lamport = doc.marks.iter().map(|m| m.lamport).max().unwrap_or(0);
        Ok(doc)
    }
}

impl Doc {
    /// The operations section: marks, attribute writes and deletes, integrated in order.
    fn load_ops(
        &mut self,
        r: &mut Reader<'_>,
        sv: &StateVector,
        clients: &[(ClientId, u64)],
        placed: &Placed,
    ) -> Result<(), DecodeError> {
        let mut ignored = Touched::default();
        let mut ops = OpReader::new(clients);
        for _ in 0..r.usize("operation count")? {
            match ops.read(r, placed)? {
                Read::One(id, op) => self.load_op(sv, id, op, &mut ignored)?,
                Read::Builtin(at, deps, rank) => {
                    for (key, write) in builtin::writes(at, &deps, rank) {
                        let id = write.id;
                        self.load_op(sv, id, Op::Attr(key, write), &mut ignored)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Integrates one operation of a snapshot, refusing a dependency outside it or a clock
    /// already taken.
    fn load_op(
        &mut self,
        sv: &StateVector,
        id: Id,
        op: Op<'_>,
        touched: &mut Touched,
    ) -> Result<(), DecodeError> {
        if op.first_missing(|d| sv.contains(d)).is_some() {
            return Err(bad("a dependency outside the snapshot"));
        }
        if self.entry_at(id).is_some() {
            return Err(bad("overlapping operations"));
        }
        self.integrate(id, op, touched)
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
                Entry::Mark(_) | Entry::Attr(_) | Entry::Delete(_) => 1,
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

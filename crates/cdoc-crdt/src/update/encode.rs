//! Encoding the operations a peer lacks. Pieces of one original run that are still chained
//! (each piece's left origin the previous piece's last unit, one right origin, both visible
//! text or both tombstones) travel as one operation, so the encoding depends on what a replica
//! holds and not on how its items happen to be split.

use super::ops::{Body, ItemOp};
use super::wire::{write_attr, write_delete, write_header, write_item, write_mark};
use super::wire::{ATTR, DELETE, KIND_UPDATE, MARK, SKIP};
use crate::bytes::Writer;
use crate::doc::{ClientLog, Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use crate::item::{byte_prefix, Content};
use std::borrow::Cow;

/// An operation being accumulated before it is written; its text borrows the document's.
enum Open<'d> {
    Item { clock: u64, op: ItemOp<'d> },
    Skip { len: u64 },
}

/// Where a snapshot's items go: each prefixed by its client's index in `clients`, text to
/// `section`.
pub(crate) struct SnapshotOut<'s> {
    pub(crate) clients: &'s [ClientId],
    pub(crate) section: &'s mut String,
}

/// Writes operations, merging what can be merged: one client's in clock order for an update,
/// every client's items in document order for a snapshot.
pub(crate) struct OpSink<'s, 'd> {
    client: ClientId,
    pub(crate) w: Writer,
    pub(crate) count: usize,
    open: Option<Open<'d>>,
    snapshot: Option<SnapshotOut<'s>>,
    /// The document's text buffer, which every borrowed text body is a slice of.
    text: &'d str,
}

/// Where `part` starts in `whole`, when it is a slice of it.
fn offset_in(whole: &str, part: &str) -> Option<usize> {
    let at = (part.as_ptr() as usize).checked_sub(whole.as_ptr() as usize)?;
    (at + part.len() <= whole.len()).then_some(at)
}

impl<'s, 'd> OpSink<'s, 'd> {
    pub(crate) const fn new(
        client: ClientId,
        text: &'d str,
        snapshot: Option<SnapshotOut<'s>>,
    ) -> Self {
        Self {
            client,
            w: Writer::new(),
            count: 0,
            open: None,
            snapshot,
            text,
        }
    }

    /// Switches to another client's operations (snapshots).
    pub(crate) fn set_client(&mut self, client: ClientId) {
        if client != self.client {
            self.flush();
            self.client = client;
        }
    }

    pub(crate) fn flush(&mut self) {
        match self.open.take() {
            Some(Open::Item { clock, op }) => {
                let own = Id::new(self.client, clock);
                let section = match &mut self.snapshot {
                    Some(out) => {
                        let index = out.clients.binary_search(&self.client);
                        self.w.usize(index.expect("every client is in the table"));
                        self.w.uvar(clock);
                        Some(&mut *out.section)
                    }
                    None => None,
                };
                write_item(&mut self.w, own, &op, section);
            }
            Some(Open::Skip { len }) => {
                self.w.u8(SKIP);
                self.w.uvar(len);
            }
            None => return,
        }
        self.count += 1;
    }

    pub(crate) fn item(&mut self, clock: u64, op: ItemOp<'d>) {
        if let Some(Open::Item { clock: c, op: open }) = &mut self.open {
            let chained = *c + u64::from(open.len) == clock
                && op.origin_left == Some(Id::new(self.client, clock - 1))
                && op.origin_right == open.origin_right;
            match (&mut open.body, &op.body) {
                (Body::Text(a), Body::Text(b)) if chained => {
                    // Pieces of one run are adjacent in the text buffer: widen the slice.
                    let span = match (&*a, b) {
                        (Cow::Borrowed(x), Cow::Borrowed(y)) => offset_in(self.text, x)
                            .zip(offset_in(self.text, y))
                            .filter(|&(i, j)| i + x.len() == j)
                            .map(|(i, j)| i..j + y.len()),
                        _ => None,
                    };
                    match span {
                        Some(span) => *a = Cow::Borrowed(&self.text[span]),
                        None => a.to_mut().push_str(b),
                    }
                    open.len += op.len;
                    return;
                }
                (Body::Gone, Body::Gone) if chained => {
                    open.len += op.len;
                    return;
                }
                _ => {}
            }
        }
        self.flush();
        self.open = Some(Open::Item { clock, op });
    }

    pub(crate) fn skip(&mut self, len: u64) {
        if let Some(Open::Skip { len: open }) = &mut self.open {
            *open += len;
            return;
        }
        self.flush();
        self.open = Some(Open::Skip { len });
    }
}

impl Doc {
    /// Item `idx` as an operation, without its first `skip` units.
    pub(crate) fn item_op(&self, idx: u32, skip: u64) -> ItemOp<'_> {
        let item = self.seq.item(idx);
        let skip32 = u32::try_from(skip).expect("skip inside the item");
        let body = match item.content {
            Content::Text { start, bytes } => {
                let run = &self.text[start as usize..(start + bytes) as usize];
                Body::Text(Cow::Borrowed(&run[byte_prefix(run, skip32 as usize)..]))
            }
            Content::Atom { index, kind } => {
                Body::Atom(kind, Cow::Borrowed(self.atom_bytes(index)))
            }
            Content::Gone => Body::Gone,
        };
        ItemOp {
            len: item.len - skip32,
            origin_left: if skip > 0 {
                Some(item.id.offset(skip - 1))
            } else {
                item.origin_left
            },
            origin_right: item.origin_right,
            body,
        }
    }

    /// Writes `log`'s operations from clock `from` on into `sink`.
    fn emit_log<'d>(&'d self, sink: &mut OpSink<'_, 'd>, log: &ClientLog, from: u64) {
        let first = log.entries.floor(from).map_or(from, |(k, _)| k);
        for (clock, entry) in log.entries.iter_from(first) {
            match entry {
                Entry::Item(idx) => {
                    let skip = from.saturating_sub(clock);
                    if skip < u64::from(self.seq.item(idx).len) {
                        sink.item(clock + skip, self.item_op(idx, skip));
                    }
                }
                Entry::Mark(m) if clock >= from => {
                    sink.flush();
                    sink.w.u8(MARK);
                    write_mark(&mut sink.w, &self.marks[m as usize]);
                    sink.count += 1;
                }
                Entry::Attr(k) if clock >= from => {
                    match self.attrs.surviving(k, Id::new(sink.client, clock)) {
                        Some(write) => {
                            sink.flush();
                            sink.w.u8(ATTR);
                            write_attr(&mut sink.w, self.attrs.key(k), write);
                            sink.count += 1;
                        }
                        None => sink.skip(1),
                    }
                }
                Entry::Delete(k) if clock >= from => {
                    sink.flush();
                    sink.w.u8(DELETE);
                    write_delete(&mut sink.w, self.delete_op(k));
                    sink.count += 1;
                }
                Entry::Skip(n) if clock + n > from => sink.skip(clock + n - from.max(clock)),
                Entry::Mark(_) | Entry::Attr(_) | Entry::Delete(_) | Entry::Skip(_) => {}
            }
        }
        sink.flush();
    }

    /// Everything this replica holds that a replica at `sv` does not: the operations past
    /// `sv`, deletions included (each is an operation of its deleter, so only the ones `sv`
    /// lacks travel). The update carries this replica's GC floor; a replica that does not hold
    /// all of it refuses the update with [`DecodeError::Compacted`](crate::DecodeError).
    #[must_use]
    pub fn encode_diff(&self, sv: &StateVector) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, KIND_UPDATE);
        self.floor.write(&mut w);
        let clients: Vec<_> = self
            .logs
            .iter()
            .filter(|(&c, log)| log.next > sv.get(c))
            .collect();
        w.usize(clients.len());
        for (&client, log) in clients {
            let from = sv.get(client);
            let mut sink = OpSink::new(client, &self.text, None);
            self.emit_log(&mut sink, log, from);
            w.uvar(client.0);
            w.uvar(from);
            w.usize(sink.count);
            w.raw(&sink.w.finish());
        }
        w.finish()
    }
}

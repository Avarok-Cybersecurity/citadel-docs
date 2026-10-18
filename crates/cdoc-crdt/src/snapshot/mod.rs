//! Snapshots: the full state of a replica, written in document order so loading is a single
//! pass that appends items without integrating them, and a text section that is copied into
//! the replica's text buffer in one piece.

mod decode;

use crate::bytes::{DecodeError, Reader, Writer};
use crate::doc::{Doc, Entry};
use crate::gc::Plan;
use crate::id::{ClientId, Id, StateVector};
use crate::update::wire::{read_header, write_attr, write_delete, write_header, write_mark};
use crate::update::wire::{ATTR, DELETE, KIND_SNAPSHOT, MARK};
use crate::update::{OpSink, SnapshotOut};

impl Doc {
    /// The full state: every item (tombstones without content), mark, surviving attribute
    /// write and delete operation, and the GC floor. Two replicas that hold the same
    /// operations (and floor) produce identical bytes.
    #[must_use]
    pub fn encode_snapshot(&self) -> Vec<u8> {
        self.snapshot_bytes(None)
    }

    /// The snapshot, without what `gc` collects.
    pub(crate) fn snapshot_bytes(&self, gc: Option<&Plan>) -> Vec<u8> {
        let sv = self.state_vector();
        let clients: Vec<ClientId> = sv.iter().map(|(c, _)| c).collect();
        let mut text = String::new();
        let (count, items) = {
            let out = SnapshotOut {
                clients: &clients,
                section: &mut text,
            };
            let mut sink = OpSink::new(ClientId(0), &self.text, Some(out));
            for idx in self.seq.iter() {
                let item = self.seq.item(idx);
                sink.set_client(item.id.client);
                match gc.and_then(|plan| plan.kept.get(&idx)) {
                    None => sink.item(item.id.clock, self.item_op(idx, 0)),
                    Some(kept) => {
                        for &(a, b) in kept {
                            let mut op = self.item_op(idx, a);
                            op.len = u32::try_from(b - a).expect("inside the item");
                            sink.item(item.id.clock + a, op);
                        }
                    }
                }
            }
            sink.flush();
            (sink.count, sink.w.finish())
        };
        let mut w = Writer::new();
        write_header(&mut w, KIND_SNAPSHOT);
        sv.write(&mut w);
        gc.map_or(&self.floor, |plan| &plan.floor).write(&mut w);
        w.str(&text);
        w.usize(count);
        w.raw(&items);
        let mut others = Writer::new();
        let mut n = 0usize;
        for (ci, (&client, log)) in self.logs.iter().enumerate() {
            // Each operation's clock is a gap from the end of the client's previous one.
            let mut next = 0u64;
            for (clock, entry) in log.entries.iter() {
                let mut at = |w: &mut Writer, tag: u8| {
                    w.usize(ci);
                    w.uvar(clock - next);
                    w.u8(tag);
                    next = clock + 1;
                };
                match entry {
                    Entry::Mark(m) => {
                        at(&mut others, MARK);
                        write_mark(&mut others, &self.marks[m as usize]);
                    }
                    Entry::Attr(k) => {
                        let Some(write) = self.attrs.surviving(k, Id::new(client, clock)) else {
                            continue;
                        };
                        at(&mut others, ATTR);
                        write_attr(&mut others, self.attrs.key(k), write);
                    }
                    Entry::Delete(_)
                        if gc.is_some_and(|plan| plan.drops_delete(Id::new(client, clock))) =>
                    {
                        continue
                    }
                    Entry::Delete(k) => {
                        at(&mut others, DELETE);
                        write_delete(&mut others, self.delete_op(k));
                    }
                    Entry::Item(_) | Entry::Skip(_) => continue,
                }
                n += 1;
            }
        }
        w.usize(n);
        w.raw(&others.finish());
        w.finish()
    }
}

/// The version a snapshot holds, read without loading it.
pub fn snapshot_version(bytes: &[u8]) -> Result<StateVector, DecodeError> {
    let mut r = Reader::new(bytes);
    read_header(&mut r, KIND_SNAPSHOT)?;
    StateVector::read(&mut r)
}

/// The update that takes a replica holding snapshot `old` to snapshot `new`: what a relay
/// stores when it compacts its log. `new` must contain `old` (a later state of the same
/// document); every operation of `new` past `old`'s version, deletions included, and nothing else.
pub fn diff_snapshots(old: &[u8], new: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let base = snapshot_version(old)?;
    // The replica only encodes; it never edits, so the client it would write as is moot.
    let doc = Doc::decode_snapshot(new, ClientId(0))?;
    if !doc.state_vector().covers(&base) {
        return Err(DecodeError::Inconsistent(
            "the new snapshot does not contain the old",
        ));
    }
    Ok(doc.encode_diff(&base))
}

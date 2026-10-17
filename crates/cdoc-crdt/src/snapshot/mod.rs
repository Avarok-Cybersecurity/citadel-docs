//! Snapshots: the full state of a replica, written in document order so loading is a single
//! pass that appends items without integrating them, and a text section that is copied into
//! the replica's text buffer in one piece.

mod decode;

use crate::bytes::{DecodeError, Reader, Writer};
use crate::doc::{Doc, Entry};
use crate::id::{ClientId, StateVector};
use crate::update::wire::{read_header, write_attr, write_header, write_mark};
use crate::update::wire::{ATTR, KIND_SNAPSHOT, MARK};
use crate::update::{OpSink, SnapshotOut};

impl Doc {
    /// The full state: every item (tombstones without content), mark and surviving attribute
    /// write. Two replicas that hold the same operations produce identical bytes.
    #[must_use]
    pub fn encode_snapshot(&self) -> Vec<u8> {
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
                sink.item(item.id.clock, self.item_op(idx, 0));
            }
            sink.flush();
            (sink.count, sink.w.finish())
        };
        let mut w = Writer::new();
        write_header(&mut w, KIND_SNAPSHOT);
        sv.write(&mut w);
        w.str(&text);
        w.usize(count);
        w.raw(&items);
        let mut others = Writer::new();
        let mut n = 0usize;
        for (ci, (&client, log)) in self.logs.iter().enumerate() {
            for (clock, entry) in log.entries.iter() {
                let head = match entry {
                    Entry::Mark(m) => Some((MARK, Some(m), None)),
                    Entry::Attr(k) => self
                        .attrs
                        .surviving(k, crate::id::Id::new(client, clock))
                        .map(|w| (ATTR, None, Some((k, w)))),
                    Entry::Item(_) | Entry::Skip(_) => None,
                };
                let Some((tag, mark, attr)) = head else {
                    continue;
                };
                others.usize(ci);
                others.uvar(clock);
                others.u8(tag);
                if let Some(m) = mark {
                    write_mark(&mut others, &self.marks[m as usize]);
                }
                if let Some((k, write)) = attr {
                    write_attr(&mut others, self.attrs.key(k), write);
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
/// document); every operation of `new` past `old`'s version, and every deletion, is included.
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

//! Snapshots: the full state of a replica, written in document order so loading is a single
//! pass that appends items without integrating them, and a text section that is copied into
//! the replica's text buffer in one piece.

mod builtin;
mod decode;
mod items;
mod ops;
mod ops_read;
mod positions;

use crate::bytes::{DecodeError, Reader, Writer};
use crate::doc::Doc;
use crate::gc::Plan;
use crate::id::{ClientId, StateVector};
use crate::update::wire::{read_header, write_header, KIND_SNAPSHOT};
use crate::update::OpSink;
use items::SnapshotItems;
use ops::write_ops;
use positions::Positions;

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
        let positions = Positions::new(self, gc.map(|plan| &plan.kept));
        let mut out = SnapshotItems {
            clients: &clients,
            positions: &positions,
            section: String::new(),
            cursors: vec![0; clients.len()],
            next: 0,
        };
        let (count, items) = {
            let mut sink = OpSink::new(ClientId(0), &self.text, &mut out);
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
        let (n, others) = write_ops(self, gc, &clients, &positions);
        let mut w = Writer::new();
        write_header(&mut w, KIND_SNAPSHOT);
        sv.write(&mut w);
        gc.map_or(&self.floor, |plan| &plan.floor).write(&mut w);
        w.str(&out.section);
        w.usize(count);
        w.raw(&items);
        w.usize(n);
        w.raw(&others);
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

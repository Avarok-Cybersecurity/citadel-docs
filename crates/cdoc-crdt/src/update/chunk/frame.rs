//! The chunks being written, each measured exactly as it grows: an update is a frame (header,
//! GC floor, client count) and, per client, its id, first clock, operation count and
//! operations, so the size of a chunk after one more operation is known before it is added.

use super::super::ops::Op;
use super::super::wire::{op_size, write_header, write_op, KIND_UPDATE};
use super::ChunkError;
use crate::bytes::Writer;
use crate::id::{ClientId, Id, StateVector};
use std::collections::BTreeMap;

/// One client's operations in the open chunk.
struct Part {
    first: u64,
    count: usize,
    ops: Writer,
}

/// The bytes `Writer::uvar` writes for `value`: seven bits per byte.
const fn varint_len(value: u64) -> usize {
    let bits = 64 - (value | 1).leading_zeros() as usize;
    bits.div_ceil(7)
}

fn part_header(client: ClientId, first: u64, count: usize) -> usize {
    varint_len(client.0) + varint_len(first) + varint_len(count as u64)
}

pub(super) struct Chunks {
    /// The header and the GC floor, which every chunk starts with.
    prefix: Vec<u8>,
    max_bytes: usize,
    open: BTreeMap<ClientId, Part>,
    /// The open chunk's size as written.
    size: usize,
    done: Vec<Vec<u8>>,
}

impl Chunks {
    pub(super) fn new(floor: &StateVector, max_bytes: usize) -> Result<Self, ChunkError> {
        let mut w = Writer::new();
        write_header(&mut w, KIND_UPDATE);
        floor.write(&mut w);
        let prefix = w.finish();
        let frame = prefix.len() + varint_len(0);
        if frame > max_bytes {
            return Err(ChunkError::LimitBelowFrame { max_bytes, frame });
        }
        Ok(Self {
            prefix,
            max_bytes,
            open: BTreeMap::new(),
            size: frame,
            done: Vec::new(),
        })
    }

    pub(super) const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub(super) fn is_open_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// What adding `bytes` of operations for `client` adds to the open chunk: the bytes, and
    /// the client's part header (new part) or the growth of its operation count.
    fn growth(&self, client: ClientId, clock: u64, bytes: usize) -> usize {
        if let Some(p) = self.open.get(&client) {
            bytes + varint_len(p.count as u64 + 1) - varint_len(p.count as u64)
        } else {
            let n = self.open.len() as u64;
            bytes + part_header(client, clock, 1) + varint_len(n + 1) - varint_len(n)
        }
    }

    pub(super) fn fits(&self, client: ClientId, clock: u64, op: &Op<'_>) -> bool {
        self.size + self.growth(client, clock, op_size(Id::new(client, clock), op))
            <= self.max_bytes
    }

    /// The size of an update carrying only `op`.
    pub(super) fn alone(&self, client: ClientId, clock: u64, op: &Op<'_>) -> usize {
        self.prefix.len()
            + varint_len(1)
            + part_header(client, clock, 1)
            + op_size(Id::new(client, clock), op)
    }

    /// Adds `op` to the open chunk when it fits, written once in place and taken back when it
    /// does not; operations of one client arrive in clock order, so a part is contiguous.
    pub(super) fn try_add(&mut self, client: ClientId, clock: u64, op: &Op<'_>) -> bool {
        let framing = self.growth(client, clock, 0);
        let part = self.open.entry(client).or_insert_with(|| Part {
            first: clock,
            count: 0,
            ops: Writer::new(),
        });
        let before = part.ops.len();
        write_op(&mut part.ops, Id::new(client, clock), op);
        let size = self.size + framing + part.ops.len() - before;
        if size > self.max_bytes {
            part.ops.truncate(before);
            if part.count == 0 {
                self.open.remove(&client);
            }
            return false;
        }
        part.count += 1;
        self.size = size;
        true
    }

    /// Writes the open chunk out, when it holds anything.
    pub(super) fn close(&mut self) {
        if self.open.is_empty() {
            return;
        }
        let parts = std::mem::take(&mut self.open);
        self.done.push(self.write(parts));
        self.size = self.prefix.len() + varint_len(0);
    }

    fn write(&self, parts: BTreeMap<ClientId, Part>) -> Vec<u8> {
        let mut w = Writer::new();
        w.raw(&self.prefix);
        w.usize(parts.len());
        for (client, part) in parts {
            w.uvar(client.0);
            w.uvar(part.first);
            w.usize(part.count);
            w.raw(&part.ops.finish());
        }
        let out = w.finish();
        debug_assert_eq!(out.len(), self.size, "the chunk was measured as written");
        out
    }

    /// Every chunk; an empty diff is one update with no operations.
    pub(super) fn finish(mut self) -> Vec<Vec<u8>> {
        if self.done.is_empty() || !self.open.is_empty() {
            let parts = std::mem::take(&mut self.open);
            let last = self.write(parts);
            self.done.push(last);
        }
        self.done
    }
}

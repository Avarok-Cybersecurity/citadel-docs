//! Operations that arrived before their dependencies, and how much of them a replica holds.
//!
//! Dropping a buffered operation is always safe: it is not in the state vector, so a later
//! diff against this replica sends it again. The buffer is therefore capped, and what is over
//! the cap is dropped and counted rather than refused.

use super::ops::Op;
use super::wire::op_size;
use crate::id::{ClientId, Id};
use std::collections::BTreeMap;

/// How much a replica buffers of operations that wait for a dependency. There is no default:
/// the caller sizes it for its memory (a browser tab holds far less than a server).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingLimits {
    /// Most buffered operations.
    pub max_ops: usize,
    /// Most buffered bytes, each operation counted at its update size.
    pub max_bytes: usize,
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

    /// Drops every buffered operation; returns how many there were.
    pub(crate) fn clear(&mut self) -> u64 {
        let n = self.ops.values().map(BTreeMap::len).sum::<usize>();
        self.ops.clear();
        n as u64
    }

    /// Drops operations until the buffer is within `limits`; returns how many were dropped.
    /// The client holding the most bytes loses its highest clock first: that operation is the
    /// furthest from integrating, and a peer flooding the buffer loses its own operations
    /// before anyone else's.
    pub(crate) fn enforce(&mut self, limits: PendingLimits) -> u64 {
        let mut held: BTreeMap<ClientId, usize> = self
            .ops
            .iter()
            .map(|(&client, ops)| {
                let bytes = ops
                    .iter()
                    .map(|(&clock, op)| op_size(Id::new(client, clock), op))
                    .sum();
                (client, bytes)
            })
            .collect();
        let mut ops: usize = self.ops.values().map(BTreeMap::len).sum();
        let mut bytes: usize = held.values().sum();
        let mut dropped = 0;
        while ops > limits.max_ops || bytes > limits.max_bytes {
            let Some((&client, _)) = held.iter().max_by_key(|(_, &b)| b) else {
                break;
            };
            let Some(queue) = self.ops.get_mut(&client) else {
                break;
            };
            let Some((clock, op)) = queue.pop_last() else {
                break;
            };
            let size = op_size(Id::new(client, clock), &op);
            if let Some(h) = held.get_mut(&client) {
                *h -= size;
            }
            ops -= 1;
            bytes -= size;
            dropped += 1;
            if queue.is_empty() {
                self.ops.remove(&client);
                held.remove(&client);
            }
        }
        dropped
    }
}

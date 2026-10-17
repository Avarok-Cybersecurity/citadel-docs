//! Applying an update: integrate what can be, buffer what waits for a dependency, report what
//! is missing and what changed.

use super::decode::decode;
use super::ops::Op;
use crate::bytes::DecodeError;
use crate::changes::{Changes, Touched};
use crate::doc::Doc;
use crate::id::{ClientId, Id};
use std::collections::{BTreeMap, VecDeque};

/// The outcome of [`Doc::apply_update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// What changed.
    pub changes: Changes,
    /// Units the buffered remainder waits for, one per blocked client; empty when everything
    /// was integrated. Ask a peer for a diff against this replica's state vector to fill them.
    pub missing: Vec<Id>,
}

type Queue<'a> = VecDeque<(u64, Op<'a>)>;

impl Doc {
    /// Applies an update (or a snapshot's diff). Operations already held are ignored, so
    /// applying twice changes nothing; operations whose dependencies are absent wait in a
    /// buffer and are integrated by a later update that supplies them.
    ///
    /// An update that fails to decode changes nothing. One that decodes but contradicts this
    /// replica (an origin that is not an item) is an error, and what was integrated before the
    /// contradiction stays.
    ///
    /// # Panics
    /// Never: an operation stays queued only when it lacks a clock or a dependency.
    pub fn apply_update(&mut self, bytes: &[u8]) -> Result<Applied, DecodeError> {
        let decoded = decode(bytes)?;
        let mut queues: BTreeMap<ClientId, Queue<'_>> = BTreeMap::new();
        for (client, ops) in std::mem::take(&mut self.pending.ops) {
            queues.entry(client).or_default().extend(ops);
        }
        for (client, ops) in decoded.clients {
            let queue = queues.entry(client).or_default();
            let merge = !queue.is_empty();
            queue.extend(ops);
            if merge {
                queue.make_contiguous().sort_by_key(|(clock, _)| *clock);
            }
        }
        let mut touched = Touched::default();
        let mut failure = None;
        'rounds: loop {
            let mut progress = false;
            for (&client, queue) in &mut queues {
                match self.drain(client, queue, &mut touched) {
                    Ok(p) => progress |= p,
                    Err(e) => {
                        failure = Some(e);
                        break 'rounds;
                    }
                }
            }
            if !progress {
                break;
            }
        }
        if let Some(e) = failure {
            // What was buffered stays buffered; this update's deletes wait with it.
            for (client, queue) in queues {
                let held = self.pending.ops.entry(client).or_default();
                held.extend(queue.into_iter().map(|(c, op)| (c, op.into_owned())));
            }
            self.pending.deletes.extend(decoded.deletes);
            return Err(e);
        }
        let mut missing = Vec::new();
        for (client, queue) in queues {
            if let Some((clock, op)) = queue.front() {
                let next = self.next_clock(client);
                missing.push(if *clock > next {
                    Id::new(client, next)
                } else {
                    self.missing_dep(op)
                        .expect("a blocked operation lacks something")
                });
            }
            if !queue.is_empty() {
                let held = self.pending.ops.entry(client).or_default();
                held.extend(queue.into_iter().map(|(c, op)| (c, op.into_owned())));
            }
        }
        let mut deletes = std::mem::take(&mut self.pending.deletes);
        deletes.extend(decoded.deletes);
        deletes.sort_unstable();
        deletes.dedup();
        for (client, start, end) in deletes {
            self.delete_range(client, start, end, &mut touched.items);
            let held = self.next_clock(client);
            if end > held {
                self.pending.deletes.push((client, start.max(held), end));
                missing.push(Id::new(client, held));
            }
        }
        missing.sort_unstable();
        missing.dedup();
        Ok(Applied {
            changes: self.changes(touched),
            missing,
        })
    }

    /// Integrates `client`'s queued operations while they are next in line and ready.
    fn drain(
        &mut self,
        client: ClientId,
        queue: &mut Queue<'_>,
        touched: &mut Touched,
    ) -> Result<bool, DecodeError> {
        let mut progress = false;
        while let Some((clock, op)) = queue.front() {
            let next = self.next_clock(client);
            let end = clock + op.len();
            if end <= next {
                queue.pop_front();
                continue;
            }
            if *clock > next || self.missing_dep(op).is_some() {
                break;
            }
            let (clock, op) = queue.pop_front().expect("front exists");
            let op = if clock < next {
                op.trim(Id::new(client, clock), next - clock)
            } else {
                op
            };
            self.integrate(Id::new(client, next), op, touched)?;
            progress = true;
        }
        Ok(progress)
    }
}

//! Reading the snapshot's operations section (layout in `ops.rs`).

use super::builtin;
use super::ops::{ATTR, BUILTIN, DELETE, LONG, MARK, SWITCH, TAG_BITS};
use super::positions::Placed;
use crate::bytes::{DecodeError, Reader};
use crate::id::{ClientId, Id, StateVector};
use crate::update::ops::{DelRange, Op};
use crate::update::wire::{push_canonical, read_attr, read_mark};

/// One operation of the section as read.
pub(super) enum Read<'a> {
    One(Id, Op<'a>),
    /// The built-in styles run at this id: what it saw and its first rank.
    Builtin(Id, StateVector, u64),
}

/// Reads one operation of the section: its id and the operation, after any client switches.
pub(super) struct OpReader<'c> {
    clients: &'c [(ClientId, u64)],
    at_client: usize,
    next: u64,
}

fn bad(what: &'static str) -> DecodeError {
    DecodeError::Inconsistent(what)
}

impl<'c> OpReader<'c> {
    pub(super) const fn new(clients: &'c [(ClientId, u64)]) -> Self {
        Self {
            clients,
            at_client: 0,
            next: 0,
        }
    }

    pub(super) fn read<'a>(
        &mut self,
        r: &mut Reader<'a>,
        placed: &Placed,
    ) -> Result<Read<'a>, DecodeError> {
        let (gap, tag) = loop {
            let head = r.uvar()?;
            let (value, tag) = (head >> TAG_BITS, head & ((1 << TAG_BITS) - 1));
            if tag != SWITCH {
                break (if value == LONG { r.uvar()? } else { value }, tag);
            }
            let step = usize::try_from(value).map_err(|_| bad("client index"))?;
            if step == 0 {
                return Err(bad("a switch to the same client"));
            }
            self.at_client = self
                .at_client
                .checked_add(step)
                .ok_or(bad("client index"))?;
            self.next = 0;
        };
        let &(owner, end) = self
            .clients
            .get(self.at_client)
            .ok_or(bad("client index"))?;
        let clock = self
            .next
            .checked_add(gap)
            .ok_or(DecodeError::OutOfRange("clock"))?;
        let len = if tag == BUILTIN { builtin::len() } else { 1 };
        self.next = clock
            .checked_add(len)
            .ok_or(DecodeError::OutOfRange("clock"))?;
        if self.next > end {
            return Err(bad("an operation past its client's clock"));
        }
        let id = Id::new(owner, clock);
        Ok(match tag {
            MARK => Read::One(id, Op::Mark(read_mark(r, id)?)),
            ATTR => {
                let (key, write) = read_attr(r, id)?;
                Read::One(id, Op::Attr(key, write))
            }
            DELETE => Read::One(id, Op::Delete(self.read_ranges(r, placed)?)),
            BUILTIN => {
                let deps = StateVector::read(r)?;
                if deps.get(owner) != clock {
                    return Err(bad("built-in styles that did not see their own clock"));
                }
                Read::Builtin(id, deps, r.uvar()?)
            }
            tag => {
                return Err(DecodeError::BadTag {
                    what: "snapshot operation",
                    tag: u8::try_from(tag).expect("three bits"),
                })
            }
        })
    }

    fn read_ranges(
        &self,
        r: &mut Reader<'_>,
        placed: &Placed,
    ) -> Result<Vec<DelRange>, DecodeError> {
        let head = r.uvar()?;
        let explicit = head & 1 == 1;
        let k = usize::try_from(head >> 1).map_err(|_| bad("delete range count"))?;
        if k == 0 {
            return Err(bad("a delete of nothing"));
        }
        let mut out = Vec::with_capacity(k.min(r.remaining()));
        let mut cursor = 0u64;
        for _ in 0..k {
            let form = if explicit { r.u8()? } else { 0 };
            let start = match form {
                0 => {
                    // Reinterpreting the zigzag's output back to a wrapping difference.
                    #[allow(clippy::cast_sign_loss)]
                    let pos = cursor.wrapping_add(r.zvar()? as u64);
                    Some(pos)
                }
                1 => None,
                tag => {
                    return Err(DecodeError::BadTag {
                        what: "delete range",
                        tag,
                    })
                }
            };
            let id = if let Some(pos) = start {
                placed.id_at(pos)?
            } else {
                let ci = r.usize("client index")?;
                let &(client, _) = self.clients.get(ci).ok_or(bad("client index"))?;
                Id::new(client, r.uvar()?)
            };
            let len = r.uvar()?;
            let end = id
                .clock
                .checked_add(len)
                .ok_or(DecodeError::OutOfRange("clock"))?;
            if let Some(pos) = start {
                cursor = pos.wrapping_add(len);
            }
            push_canonical(
                &mut out,
                DelRange {
                    client: id.client,
                    start: id.clock,
                    end,
                },
            )?;
        }
        Ok(out)
    }
}

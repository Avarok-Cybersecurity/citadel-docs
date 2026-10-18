//! Decoding an update into operations that borrow its bytes.

use super::ops::Op;
use super::wire::{read_attr, read_delete, read_header, read_item, read_mark, tag_of};
use super::wire::{ATOM, ATTR, DELETE, GONE, KIND_UPDATE, MARK, SKIP, TEXT};
use crate::bytes::{DecodeError, Reader};
use crate::id::{ClientId, Id, StateVector};
use std::borrow::Cow;

/// One client's operations, each with its first clock, in clock order.
pub(crate) type ClientOps<'a> = (ClientId, Vec<(u64, Op<'a>)>);

pub(crate) struct Decoded<'a> {
    /// The sender's GC floor: a receiver that does not hold all of it cannot apply this.
    pub(crate) floor: StateVector,
    pub(crate) clients: Vec<ClientOps<'a>>,
}

/// One operation after its first byte `head`, starting at `id`.
fn read_op<'a>(r: &mut Reader<'a>, head: u8, id: Id) -> Result<Op<'a>, DecodeError> {
    Ok(match tag_of(head) {
        TEXT | ATOM | GONE => Op::Item(read_item(r, head, id, |r| {
            Ok((Cow::Borrowed(r.str()?), false))
        })?),
        MARK => Op::Mark(read_mark(r, id)?),
        ATTR => {
            let (key, write) = read_attr(r, id)?;
            Op::Attr(key, write)
        }
        DELETE => Op::Delete(read_delete(r)?),
        SKIP => match r.uvar()? {
            0 => return Err(DecodeError::Inconsistent("empty skip")),
            n => Op::Skip(n),
        },
        tag => {
            return Err(DecodeError::BadTag {
                what: "operation",
                tag,
            })
        }
    })
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Decoded<'_>, DecodeError> {
    let mut r = Reader::new(bytes);
    read_header(&mut r, KIND_UPDATE)?;
    let floor = StateVector::read(&mut r)?;
    let n = r.usize("client count")?;
    let mut clients = Vec::with_capacity(n);
    for _ in 0..n {
        let client = ClientId(r.uvar()?);
        let mut clock = r.uvar()?;
        let count = r.usize("operation count")?;
        let mut ops = Vec::with_capacity(count);
        for _ in 0..count {
            let head = r.u8()?;
            let op = read_op(&mut r, head, Id::new(client, clock))?;
            let start = clock;
            clock = clock
                .checked_add(op.len())
                .ok_or(DecodeError::OutOfRange("clock"))?;
            ops.push((start, op));
        }
        clients.push((client, ops));
    }
    r.finish()?;
    Ok(Decoded { floor, clients })
}

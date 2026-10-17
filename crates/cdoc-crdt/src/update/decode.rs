//! Decoding an update into operations that borrow its bytes.

use super::ops::Op;
use super::wire::{read_attr, read_header, read_item, read_mark, tag_of, KIND_UPDATE};
use super::wire::{ATOM, ATTR, GONE, MARK, SKIP, TEXT};
use crate::bytes::{DecodeError, Reader};
use crate::id::{ClientId, Id};
use std::borrow::Cow;

/// One client's operations, each with its first clock, in clock order.
pub(crate) type ClientOps<'a> = (ClientId, Vec<(u64, Op<'a>)>);

pub(crate) struct Decoded<'a> {
    pub(crate) clients: Vec<ClientOps<'a>>,
    pub(crate) deletes: Vec<(ClientId, u64, u64)>,
}

/// One operation after its first byte `head`, starting at `id`.
pub(crate) fn read_op<'a>(r: &mut Reader<'a>, head: u8, id: Id) -> Result<Op<'a>, DecodeError> {
    Ok(match tag_of(head) {
        TEXT | ATOM | GONE => Op::Item(read_item(r, head, id, |r| Ok(Cow::Borrowed(r.str()?)))?),
        MARK => Op::Mark(read_mark(r, id)?),
        ATTR => {
            let (key, write) = read_attr(r, id)?;
            Op::Attr(key, write)
        }
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
    let deletes = read_deletes(&mut r)?;
    r.finish()?;
    Ok(Decoded { clients, deletes })
}

fn read_deletes(r: &mut Reader<'_>) -> Result<Vec<(ClientId, u64, u64)>, DecodeError> {
    let n = r.usize("delete set size")?;
    let mut out = Vec::new();
    for _ in 0..n {
        let client = ClientId(r.uvar()?);
        let ranges = r.usize("delete range count")?;
        let mut prev = 0u64;
        for _ in 0..ranges {
            let start = prev
                .checked_add(r.uvar()?)
                .ok_or(DecodeError::OutOfRange("clock"))?;
            let end = start
                .checked_add(r.uvar()?)
                .ok_or(DecodeError::OutOfRange("clock"))?;
            out.push((client, start, end));
            prev = end;
        }
    }
    Ok(out)
}

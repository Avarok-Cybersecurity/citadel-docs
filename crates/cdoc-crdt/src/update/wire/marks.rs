//! Wire forms of marks (anchors, keys, values) and attribute writes.

use super::{read_id, write_id};
use crate::attrs::{AttrKey, AttrWrite};
use crate::bytes::{DecodeError, Reader, Writer};
use crate::id::{Id, StateVector};
use crate::marks::{Anchor, MarkKey, MarkOp, RunKey};

fn write_anchor(w: &mut Writer, a: Anchor) {
    match a {
        Anchor::Start => w.u8(0),
        Anchor::End => w.u8(1),
        Anchor::Before(id) => {
            w.u8(2);
            write_id(w, id);
        }
        Anchor::After(id) => {
            w.u8(3);
            write_id(w, id);
        }
    }
}

fn read_anchor(r: &mut Reader<'_>) -> Result<Anchor, DecodeError> {
    Ok(match r.u8()? {
        0 => Anchor::Start,
        1 => Anchor::End,
        2 => Anchor::Before(read_id(r)?),
        3 => Anchor::After(read_id(r)?),
        tag => {
            return Err(DecodeError::BadTag {
                what: "anchor",
                tag,
            })
        }
    })
}

const LINK: u8 = 12;
const REVISION: u8 = 13;
const COMMENT: u8 = 14;

/// A mark without its id (the position gives it).
pub(crate) fn write_mark(w: &mut Writer, m: &MarkOp) {
    w.uvar(m.lamport);
    write_anchor(w, m.start);
    write_anchor(w, m.end);
    match &m.key {
        MarkKey::Run(k) => w.u8(*k as u8),
        MarkKey::Link => w.u8(LINK),
        MarkKey::Revision => w.u8(REVISION),
        MarkKey::Comment(id) => {
            w.u8(COMMENT);
            w.str(id);
        }
    }
    match &m.value {
        Some(v) => {
            w.u8(1);
            w.bytes(v);
        }
        None => w.u8(0),
    }
}

pub(crate) fn read_mark(r: &mut Reader<'_>, id: Id) -> Result<MarkOp, DecodeError> {
    let lamport = r.uvar()?;
    let start = read_anchor(r)?;
    let end = read_anchor(r)?;
    let key = match r.u8()? {
        LINK => MarkKey::Link,
        REVISION => MarkKey::Revision,
        COMMENT => MarkKey::Comment(r.str()?.to_owned()),
        code => MarkKey::Run(RunKey::from_code(code).ok_or(DecodeError::BadTag {
            what: "mark key",
            tag: code,
        })?),
    };
    let value = match r.u8()? {
        0 => None,
        1 => Some(r.bytes()?.to_vec()),
        tag => {
            return Err(DecodeError::BadTag {
                what: "mark value",
                tag,
            })
        }
    };
    Ok(MarkOp {
        id,
        lamport,
        start,
        end,
        key,
        value,
    })
}

pub(crate) fn write_attr(w: &mut Writer, key: &AttrKey, write: &AttrWrite) {
    match key {
        AttrKey::Name(name) => {
            w.u8(0);
            w.str(name);
        }
        AttrKey::Field(block, path) => {
            w.u8(1);
            write_id(w, *block);
            w.str(path);
        }
        AttrKey::Comment(id) => {
            w.u8(2);
            w.str(id);
        }
    }
    write.deps.write(w);
    w.bytes(&write.value);
}

pub(crate) fn read_attr(r: &mut Reader<'_>, id: Id) -> Result<(AttrKey, AttrWrite), DecodeError> {
    let key = match r.u8()? {
        0 => AttrKey::Name(r.str()?.to_owned()),
        1 => AttrKey::Field(read_id(r)?, r.str()?.to_owned()),
        2 => AttrKey::Comment(r.str()?.to_owned()),
        tag => {
            return Err(DecodeError::BadTag {
                what: "attribute key",
                tag,
            })
        }
    };
    let deps = StateVector::read(r)?;
    if deps.get(id.client) > id.clock {
        return Err(DecodeError::Inconsistent("a write depends on itself"));
    }
    let value = r.bytes()?.to_vec();
    Ok((key, AttrWrite { id, deps, value }))
}

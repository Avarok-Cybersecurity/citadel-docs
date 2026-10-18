//! Wire forms of single operations, shared by updates and snapshots. The layout is documented
//! in the crate root ("Binary format").

mod deletes;
mod marks;

pub(crate) use deletes::{push_canonical, read_delete, write_delete};
pub(crate) use marks::{read_attr, read_mark, write_attr, write_mark};

use super::ops::{Body, ItemOp, Op};
use crate::bytes::{DecodeError, Reader, Writer};
use crate::id::{ClientId, Id};
use crate::item::AtomKind;
use std::borrow::Cow;

pub(crate) const MAGIC: &[u8; 3] = b"CDC";
/// The format version. 2: deletions are clocked operations, block properties are per field,
/// comments are per id, and updates and snapshots carry the GC floor.
pub(crate) const VERSION: u8 = 2;
pub(crate) const KIND_UPDATE: u8 = 0;
/// A snapshot in layout 2 (positions for ids, gap-coded clocks: `snapshot/items.rs`).
pub(crate) const KIND_SNAPSHOT: u8 = 2;
/// A snapshot in layout 1 (absolute ids), which this build refuses by name.
const KIND_SNAPSHOT_LAYOUT_1: u8 = 1;

pub(crate) const TEXT: u8 = 0;
pub(crate) const ATOM: u8 = 1;
pub(crate) const GONE: u8 = 2;
pub(crate) const MARK: u8 = 3;
pub(crate) const ATTR: u8 = 4;
pub(crate) const SKIP: u8 = 5;
pub(crate) const DELETE: u8 = 6;
const TAG_MASK: u8 = 0x07;
const HAS_OL: u8 = 0x08;
const OL_PREV: u8 = 0x10;
const OL_OWN: u8 = 0x20;
const HAS_OR: u8 = 0x40;
const OR_OWN: u8 = 0x80;

pub(crate) fn write_header(w: &mut Writer, kind: u8) {
    w.raw(MAGIC);
    w.u8(VERSION);
    w.u8(kind);
}

pub(crate) fn read_header(r: &mut Reader<'_>, kind: u8) -> Result<(), DecodeError> {
    if r.raw(MAGIC.len())? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    match r.u8()? {
        VERSION => {}
        found => {
            return Err(DecodeError::Version {
                found,
                supported: VERSION,
            })
        }
    }
    match r.u8()? {
        found if found == kind => Ok(()),
        KIND_SNAPSHOT_LAYOUT_1 if kind == KIND_SNAPSHOT => Err(DecodeError::SnapshotLayout {
            found: 1,
            supported: 2,
        }),
        _ => Err(DecodeError::BadMagic),
    }
}

pub(crate) const fn tag_of(byte: u8) -> u8 {
    byte & TAG_MASK
}

pub(crate) fn write_id(w: &mut Writer, id: Id) {
    w.uvar(id.client.0);
    w.uvar(id.clock);
}

pub(crate) fn read_id(r: &mut Reader<'_>) -> Result<Id, DecodeError> {
    Ok(Id::new(ClientId(r.uvar()?), r.uvar()?))
}

/// An item whose first unit is `own`, in an update (text inline).
pub(crate) fn write_item(w: &mut Writer, own: Id, op: &ItemOp<'_>) {
    let prev = own.clock.checked_sub(1).map(|c| Id::new(own.client, c));
    let mut flags = 0;
    if let Some(ol) = op.origin_left {
        flags |= HAS_OL;
        if Some(ol) == prev {
            flags |= OL_PREV;
        } else if ol.client == own.client {
            flags |= OL_OWN;
        }
    }
    if let Some(or) = op.origin_right {
        flags |= HAS_OR;
        if or.client == own.client {
            flags |= OR_OWN;
        }
    }
    w.u8(body_tag(&op.body) | flags);
    match op.origin_left {
        Some(_) if flags & OL_PREV != 0 => {}
        Some(ol) if flags & OL_OWN != 0 => w.uvar(ol.clock),
        Some(ol) => write_id(w, ol),
        None => {}
    }
    match op.origin_right {
        Some(or) if flags & OR_OWN != 0 => w.uvar(or.clock),
        Some(or) => write_id(w, or),
        None => {}
    }
    write_body(w, op, None);
}

/// The tag of an item with this body.
pub(crate) const fn body_tag(body: &Body<'_>) -> u8 {
    match body {
        Body::Text(_) => TEXT,
        Body::Atom(..) => ATOM,
        Body::Gone => GONE,
    }
}

/// An item's body after its origins: text inline, or (snapshots) to `section` with only its
/// byte length inline.
pub(crate) fn write_body(w: &mut Writer, op: &ItemOp<'_>, section: Option<&mut String>) {
    match &op.body {
        Body::Text(text) => {
            w.uvar(op.len.into());
            match section {
                Some(out) => {
                    w.usize(text.len());
                    out.push_str(text);
                }
                None => w.str(text),
            }
        }
        Body::Atom(kind, payload) => {
            w.u8(kind.code());
            w.bytes(payload);
        }
        Body::Gone => w.uvar(op.len.into()),
    }
}

/// One decoded operation whose first unit is `own`, in an update's form (text inline).
pub(crate) fn write_op(w: &mut Writer, own: Id, op: &Op<'_>) {
    match op {
        Op::Item(item) => write_item(w, own, item),
        Op::Mark(m) => {
            w.u8(MARK);
            write_mark(w, m);
        }
        Op::Attr(key, write) => {
            w.u8(ATTR);
            write_attr(w, key, write);
        }
        Op::Delete(ranges) => {
            w.u8(DELETE);
            write_delete(w, ranges);
        }
        Op::Skip(len) => {
            w.u8(SKIP);
            w.uvar(*len);
        }
    }
}

/// Reads an item after its first byte `head`; `text` reads its text (inline in an update,
/// from the text section in a snapshot) and says whether it is known to be ASCII, in which
/// case its length in characters is its length in bytes and is checked without a scan.
pub(crate) fn read_item<'a>(
    r: &mut Reader<'a>,
    head: u8,
    own: Id,
    text: impl FnOnce(&mut Reader<'a>) -> Result<(Cow<'a, str>, bool), DecodeError>,
) -> Result<ItemOp<'a>, DecodeError> {
    let origin_left = if head & HAS_OL == 0 {
        None
    } else if head & OL_PREV != 0 {
        let clock = own.clock.checked_sub(1);
        Some(Id::new(
            own.client,
            clock.ok_or(DecodeError::Inconsistent("origin before 0"))?,
        ))
    } else if head & OL_OWN != 0 {
        Some(Id::new(own.client, r.uvar()?))
    } else {
        Some(read_id(r)?)
    };
    let origin_right = if head & HAS_OR == 0 {
        None
    } else if head & OR_OWN != 0 {
        Some(Id::new(own.client, r.uvar()?))
    } else {
        Some(read_id(r)?)
    };
    let (len, body) = read_body(r, tag_of(head), text)?;
    Ok(ItemOp {
        len,
        origin_left,
        origin_right,
        body,
    })
}

/// An item's body after its origins, by its tag; `text` as for [`read_item`].
pub(crate) fn read_body<'a>(
    r: &mut Reader<'a>,
    tag: u8,
    text: impl FnOnce(&mut Reader<'a>) -> Result<(Cow<'a, str>, bool), DecodeError>,
) -> Result<(u32, Body<'a>), DecodeError> {
    Ok(match tag {
        TEXT => {
            let len = r.u32("text length")?;
            let (body, ascii) = text(r)?;
            let chars = if ascii {
                body.len()
            } else {
                body.chars().count()
            };
            if len == 0 || chars != len as usize {
                return Err(DecodeError::Inconsistent("text length"));
            }
            (len, Body::Text(body))
        }
        ATOM => {
            let kind = AtomKind::from_code(r.u8()?)?;
            (1, Body::Atom(kind, Cow::Borrowed(r.bytes()?)))
        }
        GONE => match r.u32("deleted length")? {
            0 => return Err(DecodeError::Inconsistent("empty tombstone")),
            len => (len, Body::Gone),
        },
        tag => return Err(DecodeError::BadTag { what: "item", tag }),
    })
}

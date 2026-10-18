//! The snapshot's item layout. Items are written in document order, so an id that names a
//! unit the snapshot holds is written as where that unit sits: its distance in units (written
//! units, tombstones included) from the item. A left origin is always before its item and a
//! right origin after it, so both distances are small in practice, where the ids are random
//! clocks. An item's own clock is a gap from the end of its client's previous item.
//!
//! ```text
//! item   := client_index:varint clock:zvar head:u8 left? right? body
//!           (clock: wrapping difference from the end of this client's previous item, or 0)
//! head   := tag (bits 0-2: 0 text, 1 atom, 2 tombstone)
//!           | left (bits 3-4: 0 none, 1 the previous clock, 2 back:varint units before the
//!             item's first unit, 3 explicit)
//!           | right (bits 5-6: 0 none, 1 ahead:varint units after the item's last unit,
//!             2 explicit); bit 7 and right 3 are refused
//! explicit := client_index:varint clock:varint   (an id the snapshot holds no unit of)
//! body   := as an update's item body, a text's bytes in the text section
//! ```

use super::positions::{Placed, Positions};
use crate::bytes::{DecodeError, Reader, Writer};
use crate::id::{ClientId, Id, StateVector};
use crate::update::ops::ItemOp;
use crate::update::wire::{body_tag, write_body};
use crate::update::ItemOut;

const LEFT_SHIFT: u8 = 3;
const RIGHT_SHIFT: u8 = 5;
const LEFT_PREV: u8 = 1;
const LEFT_BACK: u8 = 2;
const LEFT_ID: u8 = 3;
const RIGHT_AHEAD: u8 = 1;
const RIGHT_ID: u8 = 2;

pub(super) fn client_index(clients: &[ClientId], client: ClientId) -> usize {
    clients
        .binary_search(&client)
        .expect("every client is in the table")
}

pub(super) fn write_explicit(w: &mut Writer, clients: &[ClientId], id: Id) {
    w.usize(client_index(clients, id.client));
    w.uvar(id.clock);
}

/// How one origin is written.
enum Coded {
    /// The previous clock: no bytes.
    Implied,
    /// Units between the origin and the item.
    Distance(u64),
    /// An id the snapshot holds no unit of.
    Explicit(Id),
}

/// Writes items in the snapshot layout, their text to `section`.
pub(super) struct SnapshotItems<'a> {
    pub(super) clients: &'a [ClientId],
    pub(super) positions: &'a Positions<'a>,
    pub(super) section: String,
    /// Per client index, the clock after its previous item.
    pub(super) cursors: Vec<u64>,
    /// The position of the next unit written.
    pub(super) next: u64,
}

impl ItemOut for &mut SnapshotItems<'_> {
    fn write(&mut self, w: &mut Writer, own: Id, op: &ItemOp<'_>) {
        let ci = client_index(self.clients, own.client);
        w.usize(ci);
        // Reinterpreting the wrapping difference as signed is the point of the zigzag.
        #[allow(clippy::cast_possible_wrap)]
        w.zvar(own.clock.wrapping_sub(self.cursors[ci]) as i64);
        let len = u64::from(op.len);
        self.cursors[ci] = own.clock.wrapping_add(len);
        // Items are written in document order, every written unit once: the next position.
        let here = self.next;
        self.next += len;
        let prev = own.clock.checked_sub(1).map(|c| Id::new(own.client, c));
        let left = op.origin_left.map(|ol| {
            if Some(ol) == prev {
                (LEFT_PREV, Coded::Implied)
            } else {
                match self.positions.of(ol).filter(|&p| p < here) {
                    Some(p) => (LEFT_BACK, Coded::Distance(here - p)),
                    None => (LEFT_ID, Coded::Explicit(ol)),
                }
            }
        });
        let right =
            op.origin_right.map(
                |or| match self.positions.of(or).filter(|&p| p >= here + len) {
                    Some(p) => (RIGHT_AHEAD, Coded::Distance(p - here - len)),
                    None => (RIGHT_ID, Coded::Explicit(or)),
                },
            );
        let mode = |m: &Option<(u8, Coded)>| m.as_ref().map_or(0, |(k, _)| *k);
        w.u8(body_tag(&op.body) | mode(&left) << LEFT_SHIFT | mode(&right) << RIGHT_SHIFT);
        for (_, coded) in left.iter().chain(&right) {
            match *coded {
                Coded::Implied => {}
                Coded::Distance(d) => w.uvar(d),
                Coded::Explicit(id) => write_explicit(w, self.clients, id),
            }
        }
        write_body(w, op, Some(&mut self.section));
    }
}

fn bad(what: &'static str) -> DecodeError {
    DecodeError::Inconsistent(what)
}

/// An item's origins as read: the right one is either explicit or `ahead` units past the
/// item's end, resolved once every item is placed.
pub(super) struct Origins {
    pub(super) left: Option<Id>,
    pub(super) right: Option<Id>,
    pub(super) ahead: Option<u64>,
}

/// Reads an item's origins after its head: the left origin, the right origin when it is
/// explicit, and the right origin's distance ahead when it is located (resolved once every
/// item is placed). `placed` holds the items before this one.
pub(super) fn read_origins(
    r: &mut Reader<'_>,
    head: u8,
    own: Id,
    placed: &Placed,
    clients: &[(ClientId, u64)],
    sv: &StateVector,
) -> Result<Origins, DecodeError> {
    let refused = || DecodeError::BadTag {
        what: "snapshot item",
        tag: head,
    };
    if head & 0x80 != 0 {
        return Err(refused());
    }
    let here = placed.end();
    let explicit = |r: &mut Reader<'_>| -> Result<Id, DecodeError> {
        let ci = r.usize("client index")?;
        let &(client, _) = clients.get(ci).ok_or(bad("client index"))?;
        let origin = Id::new(client, r.uvar()?);
        sv.contains(origin)
            .then_some(origin)
            .ok_or(bad("an origin outside the snapshot"))
    };
    let left = match (head >> LEFT_SHIFT) & 3 {
        0 => None,
        LEFT_PREV => Some(Id::new(
            own.client,
            own.clock.checked_sub(1).ok_or(bad("origin before 0"))?,
        )),
        LEFT_BACK => match r.uvar()? {
            d if d == 0 || d > here => return Err(bad("an origin outside the snapshot")),
            d => Some(placed.id_at(here - d)?),
        },
        _ => Some(explicit(r)?),
    };
    let (right, ahead) = match (head >> RIGHT_SHIFT) & 3 {
        0 => (None, None),
        RIGHT_AHEAD => (None, Some(r.uvar()?)),
        RIGHT_ID => (Some(explicit(r)?), None),
        _ => return Err(refused()),
    };
    Ok(Origins { left, right, ahead })
}

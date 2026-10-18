//! List-valued document attributes (comments, styles, numbering definitions and instances) as
//! maps keyed by each element's id: adding, editing or removing one element is one register
//! write, so concurrent changes to different elements all survive and concurrent changes to one
//! element resolve like any attribute.
//!
//! A value is `rank:varint born.client:varint born.clock:varint element`, where `element` is
//! the encoded model value, `born` the id of the write that added it and `rank` the number of
//! operations its writer held then; both are kept by every later edit, and an empty value
//! removes the element. A list is ordered by `(rank, born)`: an element added after seeing
//! another ranks after it, so for a document written by [`from_model`](super::from_model) the
//! order is the model's, and concurrent additions order the same way on every replica. A
//! reordering alone is not an edit and is not stored.

use super::BridgeError;
use crate::attrs::{AttrKey, List};
use crate::bytes::{DecodeError, Reader, Writer};
use crate::codec::{from_bytes, to_bytes};
use crate::doc::{Doc, EditError};
use crate::id::{ClientId, Id};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

/// When an element was added: its place in the list.
type Born = (u64, Id);

/// A list's elements held: encoded id to (born, encoded element).
type Held = BTreeMap<Vec<u8>, (Born, Vec<u8>)>;

/// The value of an element held: when it was added, then its encoded value.
pub(crate) fn entry_value((rank, born): Born, element: &[u8]) -> Vec<u8> {
    // Three varints of at most ten bytes each, then the element.
    let mut w = Writer::with_capacity(30 + element.len());
    w.uvar(rank);
    w.uvar(born.client.0);
    w.uvar(born.clock);
    w.raw(element);
    w.finish()
}

/// `None` for a removal, else when the element was added and its encoded value.
fn read(value: &[u8]) -> Result<Option<(Born, &[u8])>, DecodeError> {
    if value.is_empty() {
        return Ok(None);
    }
    let mut r = Reader::new(value);
    let rank = r.uvar()?;
    let born = Id::new(ClientId(r.uvar()?), r.uvar()?);
    Ok(Some(((rank, born), &value[r.position()..])))
}

impl Doc {
    /// The elements of `list` held: encoded id to (born, encoded element).
    fn held_entries(&self, list: List) -> Result<Held, DecodeError> {
        let mut out = BTreeMap::new();
        for (key, write) in self.attrs.entries(list) {
            if let Some((born, bytes)) = read(&write.value)? {
                out.insert(key.to_vec(), (born, bytes.to_vec()));
            }
        }
        Ok(out)
    }

    /// Makes `list` hold `items` (each identified by `id_of`): one write per element added,
    /// changed or removed.
    pub(crate) fn set_entries<T: Serialize, K: Serialize + Debug>(
        &mut self,
        list: List,
        items: &[T],
        id_of: impl Fn(&T) -> K,
    ) -> Result<(), EditError> {
        let mut seen = BTreeSet::new();
        let mut keyed = Vec::with_capacity(items.len());
        for item in items {
            let id = id_of(item);
            let key = to_bytes(&id)?;
            if !seen.insert(key.clone()) {
                return Err(EditError::DuplicateId {
                    element: list.element(),
                    id: format!("{id:?}"),
                });
            }
            keyed.push((key, to_bytes(item)?));
        }
        let held = self
            .held_entries(list)
            .map_err(|e| EditError::Codec(e.into()))?;
        for (key, bytes) in keyed {
            let born = match held.get(&key) {
                Some((_, old)) if *old == bytes => continue,
                Some((born, _)) => *born,
                None => {
                    let held = self.state_vector().iter().map(|(_, n)| n).sum::<u64>();
                    (held, Id::new(self.client, self.next_clock(self.client)))
                }
            };
            self.write_attr(AttrKey::Entry(list, key), entry_value(born, &bytes));
        }
        for key in held.into_keys().filter(|key| !seen.contains(key)) {
            // An empty value is a removal.
            self.write_attr(AttrKey::Entry(list, key), Vec::new());
        }
        Ok(())
    }

    /// The elements of `list`, in the order they were added.
    pub(crate) fn entry_list<T: DeserializeOwned>(
        &self,
        list: List,
    ) -> Result<Vec<T>, BridgeError> {
        let mut held: Vec<(Born, Vec<u8>)> = self
            .held_entries(list)
            .map_err(|e| BridgeError::Codec(e.into()))?
            .into_values()
            .collect();
        held.sort_by_key(|&(born, _)| born);
        held.into_iter()
            .map(|(_, bytes)| Ok(from_bytes(&bytes)?))
            .collect()
    }
}

//! Attributes: multi-value registers keyed by name (document metadata, settings, styles,
//! numbering), by one field of a structural atom's properties (`bridge/fields.rs`), or by one
//! element of a list-valued attribute, keyed by the element's id (`bridge/entries.rs`).
//!
//! A write carries the writer's state vector at the time (`deps`). A write is *dominated* when
//! another write's `deps` covers it: the other writer had seen it and overwrote it. A register
//! keeps only the writes nothing dominates; when several remain they were concurrent and the
//! one from the greatest client id wins. Dominance is transitive because a write is integrated
//! only once its `deps` are, so the result depends on the set of writes, never on the order
//! they arrived in. Dominated writes are dropped; their clocks travel as skips.

use crate::id::{Id, StateVector};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

/// A list-valued document attribute, stored element by element (`bridge/entries.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum List {
    /// `comments`, by comment id.
    Comments,
    /// `styles.styles`, by style id.
    Styles,
    /// `numbering.definitions`, by abstract numbering id.
    NumberingDefinitions,
    /// `numbering.instances`, by numbering id.
    NumberingInstances,
}

impl List {
    /// Every list, in wire-code order.
    const ALL: [Self; 4] = [
        Self::Comments,
        Self::Styles,
        Self::NumberingDefinitions,
        Self::NumberingInstances,
    ];

    /// The list with wire code `code` (its index in `ALL`).
    pub(crate) fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }

    /// The document attribute a change to the list is reported under.
    pub(crate) const fn attr(self) -> &'static str {
        match self {
            Self::Comments => crate::names::COMMENTS,
            Self::Styles => crate::names::STYLES,
            Self::NumberingDefinitions | Self::NumberingInstances => crate::names::NUMBERING,
        }
    }

    /// What one element is, for errors.
    pub(crate) const fn element(self) -> &'static str {
        match self {
            Self::Comments => "comment",
            Self::Styles => "style",
            Self::NumberingDefinitions => "numbering definition",
            Self::NumberingInstances => "numbering instance",
        }
    }
}

/// What an attribute write targets. The variant order is the index order, so the fields of
/// one block and the elements of one list are each one contiguous range of keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AttrKey {
    /// A document attribute.
    Name(String),
    /// One field (by path) of the properties of the structural atom with this id.
    Field(Id, String),
    /// The element of a list whose id encodes (`codec`) to these bytes; an empty value
    /// removes it.
    Entry(List, Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttrWrite {
    pub(crate) id: Id,
    pub(crate) deps: StateVector,
    pub(crate) value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct Attrs {
    keys: Vec<AttrKey>,
    index: BTreeMap<AttrKey, u32>,
    registers: Vec<Vec<AttrWrite>>,
}

impl Attrs {
    pub(crate) const fn new() -> Self {
        Self {
            keys: Vec::new(),
            index: BTreeMap::new(),
            registers: Vec::new(),
        }
    }

    /// The key's index, created on first use (one search, one copy of a new key).
    pub(crate) fn intern(&mut self, key: AttrKey) -> u32 {
        match self.index.entry(key) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(v) => {
                let k = u32::try_from(self.keys.len()).expect("fewer than 2^32 attribute keys");
                self.keys.push(v.key().clone());
                self.registers.push(Vec::new());
                *v.insert(k)
            }
        }
    }

    pub(crate) fn key(&self, k: u32) -> &AttrKey {
        &self.keys[k as usize]
    }

    /// Records a write; whether the register's winning write changed.
    pub(crate) fn write(&mut self, k: u32, new: AttrWrite) -> bool {
        let register = &mut self.registers[k as usize];
        if register
            .iter()
            .any(|w| w.id == new.id || w.deps.contains(new.id))
        {
            return false;
        }
        let before = winner(register).map(|w| w.id);
        register.retain(|w| !new.deps.contains(w.id));
        register.push(new);
        register.sort_by_key(|w| w.id);
        before != winner(register).map(|w| w.id)
    }

    /// The winning write of a key.
    pub(crate) fn get(&self, key: &AttrKey) -> Option<&AttrWrite> {
        let &k = self.index.get(key)?;
        winner(&self.registers[k as usize])
    }

    /// The winning writes of a block's fields, in path order.
    pub(crate) fn fields(&self, block: Id) -> impl Iterator<Item = (&str, &AttrWrite)> + '_ {
        let from = AttrKey::Field(block, String::new());
        self.winners(from).map_while(move |(key, write)| match key {
            AttrKey::Field(b, path) if *b == block => Some((path.as_str(), write)),
            _ => None,
        })
    }

    /// The winning write of every element of `list`, in key order.
    pub(crate) fn entries(&self, list: List) -> impl Iterator<Item = (&[u8], &AttrWrite)> + '_ {
        self.winners(AttrKey::Entry(list, Vec::new()))
            .map_while(move |(key, write)| match key {
                AttrKey::Entry(l, k) if *l == list => Some((k.as_slice(), write)),
                _ => None,
            })
    }

    fn winners(&self, from: AttrKey) -> impl Iterator<Item = (&AttrKey, &AttrWrite)> + '_ {
        self.index
            .range(from..)
            .filter_map(|(key, &k)| winner(&self.registers[k as usize]).map(|w| (key, w)))
    }

    /// The surviving write with this id, if it was not dominated.
    pub(crate) fn surviving(&self, k: u32, id: Id) -> Option<&AttrWrite> {
        self.registers[k as usize].iter().find(|w| w.id == id)
    }
}

fn winner(register: &[AttrWrite]) -> Option<&AttrWrite> {
    register.iter().max_by_key(|w| w.id.client)
}

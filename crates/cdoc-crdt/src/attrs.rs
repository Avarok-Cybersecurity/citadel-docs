//! Attributes: multi-value registers keyed by name (document metadata, settings, styles,
//! numbering, comments) or by a structural atom (the properties of that block).
//!
//! A write carries the writer's state vector at the time (`deps`). A write is *dominated* when
//! another write's `deps` covers it: the other writer had seen it and overwrote it. A register
//! keeps only the writes nothing dominates; when several remain they were concurrent and the
//! one from the greatest client id wins. Dominance is transitive because a write is integrated
//! only once its `deps` are, so the result depends on the set of writes, never on the order
//! they arrived in. Dominated writes are dropped; their clocks travel as skips.

use crate::id::{Id, StateVector};
use std::collections::BTreeMap;

/// What an attribute write targets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AttrKey {
    /// A document attribute.
    Name(String),
    /// The properties of the structural atom with this id.
    Block(Id),
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

    /// The key's index, created on first use.
    pub(crate) fn intern(&mut self, key: &AttrKey) -> u32 {
        if let Some(&k) = self.index.get(key) {
            return k;
        }
        let k = u32::try_from(self.keys.len()).expect("fewer than 2^32 attribute keys");
        self.keys.push(key.clone());
        self.index.insert(key.clone(), k);
        self.registers.push(Vec::new());
        k
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

    /// The surviving write with this id, if it was not dominated.
    pub(crate) fn surviving(&self, k: u32, id: Id) -> Option<&AttrWrite> {
        self.registers[k as usize].iter().find(|w| w.id == id)
    }
}

fn winner(register: &[AttrWrite]) -> Option<&AttrWrite> {
    register.iter().max_by_key(|w| w.id.client)
}

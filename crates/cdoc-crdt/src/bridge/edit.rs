//! Typed local edits: atoms, block properties, formatting and document attributes, each
//! encoded once here and stored as opaque payloads by the CRDT core.

use super::{run_props, Atom, DocAttr, Mark};
use crate::attrs::AttrKey;
use crate::codec::to_bytes;
use crate::doc::{Doc, EditError};
use crate::marks::MarkKey;
use std::ops::Range;

impl Doc {
    /// Inserts an atom before visible unit `index`.
    pub fn insert_atom(&mut self, index: usize, atom: &Atom) -> Result<(), EditError> {
        let (kind, payload) = atom.encode()?;
        self.insert_atom_raw(index, kind, payload)
    }

    /// Replaces the properties of the structural atom at `index` with those of `atom`, which
    /// must be of the same kind. Concurrent replacements of one block's properties resolve as
    /// an attribute does (see [`Doc::set_attr`]).
    pub fn set_block(&mut self, index: usize, atom: &Atom) -> Result<(), EditError> {
        let (id, found) = self.structural_at(index)?;
        let (expected, payload) = atom.encode()?;
        if expected != found || !expected.is_structural() {
            return Err(EditError::KindMismatch {
                index,
                expected,
                found,
            });
        }
        self.write_attr(&AttrKey::Block(id), payload);
        Ok(())
    }

    /// Formats the visible units `range`.
    pub fn format(&mut self, range: Range<usize>, mark: &Mark) -> Result<(), EditError> {
        match mark {
            Mark::Run(properties) => {
                let fields = run_props::decompose(properties)?;
                if fields.is_empty() {
                    return Err(EditError::Empty);
                }
                for (key, value) in fields {
                    self.mark_raw(range.clone(), MarkKey::Run(key), Some(value))?;
                }
                Ok(())
            }
            Mark::ClearRun(key) => self.mark_raw(range, MarkKey::Run(*key), None),
            Mark::Link(link) => {
                let value = link.as_ref().map(to_bytes).transpose()?;
                self.mark_raw(range, MarkKey::Link, value)
            }
            Mark::Revision(revision) => {
                let value = revision.as_ref().map(to_bytes).transpose()?;
                self.mark_raw(range, MarkKey::Revision, value)
            }
            Mark::Comment { id, anchored } => {
                let key = MarkKey::Comment(id.as_str().to_owned());
                self.mark_raw(range, key, anchored.then(Vec::new))
            }
        }
    }

    /// Writes a document attribute. Writes that saw each other: the later wins. Concurrent
    /// writes: the one from the greatest client id wins, on every replica.
    pub fn set_attr(&mut self, attr: &DocAttr) -> Result<(), EditError> {
        let value = attr.encode()?;
        self.write_attr(&AttrKey::Name(attr.name().to_owned()), value);
        Ok(())
    }
}

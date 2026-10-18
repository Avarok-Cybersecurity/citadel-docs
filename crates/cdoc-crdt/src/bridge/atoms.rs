//! Atoms decoded over one walk: each distinct structural payload once.

use super::fields::properties;
use crate::item::AtomKind;
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

/// An atom as [`Atoms`] decodes it.
pub(crate) enum Decoded {
    /// An inline atom's content.
    Inline(cdoc_model::InlineContent),
    /// A structural atom, shared by every atom with the same properties.
    Structural(Rc<super::Atom>),
}

/// [`atom`](super::fields::atom) over one walk, each distinct structural payload decoded once: a document's
/// paragraphs mostly share a handful of property sets, and decoding dominated a walk.
pub(crate) struct Atoms<'d> {
    doc: &'d crate::doc::Doc,
    decoded: HashMap<(AtomKind, Cow<'d, [u8]>), Rc<super::Atom>>,
}

impl<'d> Atoms<'d> {
    pub(crate) fn new(doc: &'d crate::doc::Doc) -> Self {
        Self {
            doc,
            decoded: HashMap::new(),
        }
    }

    /// What [`atom`](super::fields::atom) returns for the same arguments.
    pub(crate) fn get(
        &mut self,
        id: crate::id::Id,
        index: u32,
        kind: AtomKind,
    ) -> Result<Decoded, super::BridgeError> {
        let base = self.doc.atom_bytes(index);
        if !kind.is_structural() {
            return match super::Atom::decode(kind, base)? {
                super::Atom::Inline(content) => Ok(Decoded::Inline(content)),
                other => Ok(Decoded::Structural(Rc::new(other))),
            };
        }
        let key = (kind, properties(self.doc, id, kind, base)?);
        if let Some(atom) = self.decoded.get(&key) {
            return Ok(Decoded::Structural(Rc::clone(atom)));
        }
        let atom = Rc::new(super::Atom::decode(kind, &key.1)?);
        self.decoded.insert(key, Rc::clone(&atom));
        Ok(Decoded::Structural(atom))
    }
}

//! Atom payloads: one append-only buffer for all of them, so loading a snapshot copies each
//! payload into it instead of allocating one buffer per atom.

use super::Doc;

impl Doc {
    /// The payload of atom `index`.
    pub(crate) fn atom_bytes(&self, index: u32) -> &[u8] {
        let (start, len) = self.atoms[index as usize];
        &self.atom_buf[start as usize..(start + len) as usize]
    }

    /// Stores an atom payload; its index, or `None` past 4 GiB of payloads.
    pub(crate) fn push_atom(&mut self, payload: &[u8]) -> Option<u32> {
        let index = u32::try_from(self.atoms.len()).ok()?;
        let start = u32::try_from(self.atom_buf.len()).ok()?;
        let len = u32::try_from(payload.len()).ok()?;
        start.checked_add(len)?;
        self.atom_buf.extend_from_slice(payload);
        self.atoms.push((start, len));
        Some(index)
    }
}

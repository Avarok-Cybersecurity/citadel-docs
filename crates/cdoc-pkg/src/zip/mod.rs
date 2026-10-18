//! ZIP in and out. In-memory only: bytes in, bytes out. The format is small (local headers,
//! a central directory, an end record), so it is read and written here directly rather than
//! through a general-purpose archive crate: one inflate per part into an exact-size buffer,
//! one output buffer for the whole archive.

mod cp437;
mod read;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_bad;
#[cfg(test)]
mod tests_bomb;
mod write;

pub use write::DEFLATE_LEVEL;

use crate::error::PackageError;
use crate::limits::ReadLimits;
use std::collections::BTreeMap;

/// The parts of a package by name (no leading slash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    parts: BTreeMap<String, Vec<u8>>,
}

impl Package {
    /// An empty package.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            parts: BTreeMap::new(),
        }
    }

    /// Inflate an archive.
    pub fn read(bytes: &[u8], limits: &ReadLimits) -> Result<Self, PackageError> {
        read::read(bytes, limits).map(|parts| Self { parts })
    }

    /// A part's bytes.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.parts.get(name).map(Vec::as_slice)
    }

    /// Add or replace a part.
    pub fn put(&mut self, name: &str, bytes: Vec<u8>) {
        self.parts.insert(name.to_owned(), bytes);
    }

    /// Every part name.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.parts.keys().map(String::as_str)
    }

    /// Write the archive: `lead` first, then every other part deflated in name order. Entry
    /// timestamps are fixed (the format's epoch), so equal packages give equal bytes.
    pub fn write(&self, lead: Lead) -> Result<Vec<u8>, PackageError> {
        let first = self
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() == lead.name)
            .map(|(n, b)| (n.as_str(), b.as_slice(), lead.compression));
        let rest = self
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() != lead.name)
            .map(|(n, b)| (n.as_str(), b.as_slice(), Compression::Deflated));
        write::write(first.chain(rest))
    }
}

/// How an entry is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Uncompressed, as ODF requires of `mimetype` so a reader can sniff it at a fixed offset.
    Stored,
    /// Deflated.
    Deflated,
}

/// The entry a format requires at the start of the archive: `[Content_Types].xml` for OPC,
/// `mimetype` (stored) for ODF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lead {
    /// The part name.
    pub name: &'static str,
    /// How it is stored.
    pub compression: Compression,
}

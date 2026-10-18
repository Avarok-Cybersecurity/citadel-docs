//! ZIP in and out. In-memory only: bytes in, bytes out.

use crate::error::PackageError;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

/// Bounds on what a read will inflate, so a hostile archive cannot exhaust memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadLimits {
    /// Most parts in the package.
    pub max_parts: usize,
    /// Largest single part, inflated.
    pub max_part_bytes: u64,
    /// Largest total, inflated.
    pub max_total_bytes: u64,
}

impl ReadLimits {
    /// 10 000 parts, 128 MiB per part, 512 MiB in all: well above any real document.
    #[must_use]
    pub const fn generous() -> Self {
        Self {
            max_parts: 10_000,
            max_part_bytes: 128 << 20,
            max_total_bytes: 512 << 20,
        }
    }
}

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
        let zip_err = |e: zip::result::ZipError| PackageError::Zip(e.to_string());
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_err)?;
        if archive.len() > limits.max_parts {
            return Err(PackageError::TooLarge("part count"));
        }
        let mut parts = BTreeMap::new();
        let mut total = 0u64;
        for index in 0..archive.len() {
            let file = archive.by_index(index).map_err(zip_err)?;
            if file.is_dir() {
                continue;
            }
            let name = file.name().trim_start_matches('/').to_owned();
            let mut data = Vec::new();
            file.take(limits.max_part_bytes + 1)
                .read_to_end(&mut data)
                .map_err(|e| PackageError::Zip(e.to_string()))?;
            if data.len() as u64 > limits.max_part_bytes {
                return Err(PackageError::TooLarge("part size"));
            }
            total += data.len() as u64;
            if total > limits.max_total_bytes {
                return Err(PackageError::TooLarge("total size"));
            }
            parts.insert(name, data);
        }
        Ok(Self { parts })
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
        let zip_err = |e: zip::result::ZipError| PackageError::Zip(e.to_string());
        let options = |method| {
            SimpleFileOptions::default()
                .compression_method(method)
                .last_modified_time(DateTime::default())
        };
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let first = self
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() == lead.name)
            .map(|entry| (entry, lead.compression.method()));
        let rest = self
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() != lead.name)
            .map(|entry| (entry, CompressionMethod::Deflated));
        for ((name, bytes), method) in first.chain(rest) {
            writer.start_file(name, options(method)).map_err(zip_err)?;
            writer
                .write_all(bytes)
                .map_err(|e| PackageError::Zip(e.to_string()))?;
        }
        Ok(writer.finish().map_err(zip_err)?.into_inner())
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

impl Compression {
    const fn method(self) -> CompressionMethod {
        match self {
            Self::Stored => CompressionMethod::Stored,
            Self::Deflated => CompressionMethod::Deflated,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const OPC: Lead = Lead {
        name: "[Content_Types].xml",
        compression: Compression::Deflated,
    };

    #[test]
    fn content_types_is_written_first_whatever_the_other_parts_are_called() {
        let mut package = Package::empty();
        package.put("0-sorts-first", b"x".to_vec());
        package.put("[Content_Types].xml", b"<Types/>".to_vec());
        package.put("word/document.xml", b"<d/>".to_vec());
        let bytes = package.write(OPC).expect("writes");
        let archive = ZipArchive::new(Cursor::new(bytes)).expect("a zip");
        assert_eq!(archive.file_names().next(), Some("[Content_Types].xml"));
        assert_eq!(archive.len(), 3);
    }

    #[test]
    fn a_stored_lead_is_first_and_uncompressed_and_the_rest_deflated() {
        let mut package = Package::empty();
        package.put("content.xml", vec![b'x'; 200]);
        package.put(
            "mimetype",
            b"application/vnd.oasis.opendocument.text".to_vec(),
        );
        let lead = Lead {
            name: "mimetype",
            compression: Compression::Stored,
        };
        let bytes = package.write(lead).expect("writes");
        let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("a zip");
        let first = archive.by_index(0).expect("an entry");
        assert_eq!(
            (first.name(), first.compression()),
            ("mimetype", CompressionMethod::Stored)
        );
        drop(first);
        let second = archive.by_index(1).expect("an entry");
        assert_eq!(second.compression(), CompressionMethod::Deflated);
    }

    #[test]
    fn entries_are_inflated_back_unchanged() {
        let mut package = Package::empty();
        package.put("a/b.xml", "caf\u{e9}".as_bytes().to_vec());
        let bytes = package.write(OPC).expect("writes");
        let back = Package::read(&bytes, &ReadLimits::generous()).expect("reads");
        assert_eq!(back, package);
    }
}

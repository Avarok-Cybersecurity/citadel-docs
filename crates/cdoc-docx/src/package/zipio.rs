//! ZIP in and out. In-memory only: bytes in, bytes out.

use crate::error::DocxError;
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
    pub fn read(bytes: &[u8], limits: &ReadLimits) -> Result<Self, DocxError> {
        let zip_err = |e: zip::result::ZipError| DocxError::Zip(e.to_string());
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_err)?;
        if archive.len() > limits.max_parts {
            return Err(DocxError::TooLarge("part count"));
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
                .map_err(|e| DocxError::Zip(e.to_string()))?;
            if data.len() as u64 > limits.max_part_bytes {
                return Err(DocxError::TooLarge("part size"));
            }
            total += data.len() as u64;
            if total > limits.max_total_bytes {
                return Err(DocxError::TooLarge("total size"));
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

    /// Deflate to an archive with `[Content_Types].xml` first, as OPC readers expect. Entry
    /// timestamps are fixed (the format's epoch), so equal packages give equal bytes.
    pub fn write(&self) -> Result<Vec<u8>, DocxError> {
        let zip_err = |e: zip::result::ZipError| DocxError::Zip(e.to_string());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(DateTime::default());
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let first = "[Content_Types].xml";
        let ordered = self
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() == first)
            .chain(self.parts.iter().filter(|(n, _)| n.as_str() != first));
        for (name, bytes) in ordered {
            writer.start_file(name, options).map_err(zip_err)?;
            writer
                .write_all(bytes)
                .map_err(|e| DocxError::Zip(e.to_string()))?;
        }
        Ok(writer.finish().map_err(zip_err)?.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_is_written_first_whatever_the_other_parts_are_called() {
        let mut package = Package::empty();
        package.put("0-sorts-first", b"x".to_vec());
        package.put("[Content_Types].xml", b"<Types/>".to_vec());
        package.put("word/document.xml", b"<d/>".to_vec());
        let bytes = package.write().expect("writes");
        let archive = ZipArchive::new(Cursor::new(bytes)).expect("a zip");
        assert_eq!(archive.file_names().next(), Some("[Content_Types].xml"));
        assert_eq!(archive.len(), 3);
    }

    #[test]
    fn entries_are_inflated_back_unchanged() {
        let mut package = Package::empty();
        package.put("a/b.xml", "caf\u{e9}".as_bytes().to_vec());
        let bytes = package.write().expect("writes");
        let back = Package::read(&bytes, &ReadLimits::generous()).expect("reads");
        assert_eq!(back, package);
    }
}

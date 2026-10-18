//! The archive reader and writer against the `zip` crate (a test-only oracle): every archive
//! one writes, the other reads the same way, and the writer's bytes are the crate's bytes.

use super::*;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

pub(super) const OPC: Lead = Lead {
    name: "[Content_Types].xml",
    compression: Compression::Deflated,
};

/// What the `zip` crate reads: every non-directory entry, by name without a leading slash.
pub(super) fn oracle(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the oracle reads it");
    let mut parts = BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("an entry");
        if file.is_dir() {
            continue;
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect("inflates");
        parts.insert(file.name().trim_start_matches('/').to_owned(), data);
    }
    parts
}

pub(super) fn ours(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    Package::read(bytes, &ReadLimits::generous()).map(|p| p.parts)
}

/// An archive the `zip` crate writes; `stream` makes it use data descriptors.
pub(super) fn crate_archive(entries: &[(&str, &[u8], CompressionMethod)], stream: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if stream {
        let mut w = ZipWriter::new_stream(&mut out);
        add(&mut w, entries);
        w.finish().expect("finishes");
    } else {
        let mut w = ZipWriter::new(Cursor::new(&mut out));
        add(&mut w, entries);
        w.finish().expect("finishes");
    }
    out
}

fn add<W: Write + std::io::Seek>(
    w: &mut ZipWriter<W>,
    entries: &[(&str, &[u8], CompressionMethod)],
) {
    let options = |m| {
        SimpleFileOptions::default()
            .compression_method(m)
            .last_modified_time(DateTime::default())
    };
    for (name, data, method) in entries {
        if name.ends_with('/') {
            w.add_directory(*name, options(*method)).expect("dir");
        } else {
            w.start_file(*name, options(*method)).expect("starts");
            w.write_all(data).expect("writes");
        }
    }
}

fn sample() -> Package {
    let mut package = Package::empty();
    package.put("[Content_Types].xml", b"<Types/>".to_vec());
    package.put(
        "word/document.xml",
        "<d>caf\u{e9} ".repeat(500).into_bytes(),
    );
    package.put("caf\u{e9}/\u{1F600}.xml", b"<a/>".to_vec());
    package.put("empty", Vec::new());
    package.put(
        "media/noise.bin",
        (0..5000u32).map(|i| (i * 7919 % 251) as u8).collect(),
    );
    package
}

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
    let package = sample();
    let bytes = package.write(OPC).expect("writes");
    assert_eq!(
        Package::read(&bytes, &ReadLimits::generous()),
        Ok(package.clone())
    );
    assert_eq!(
        oracle(&bytes),
        package.parts,
        "the zip crate reads what we wrote"
    );
}

#[test]
fn the_writer_writes_exactly_the_bytes_the_zip_crate_wrote() {
    for lead in [
        OPC,
        Lead {
            name: "empty",
            compression: Compression::Stored,
        },
    ] {
        let package = sample();
        let mut entries: Vec<(&str, &[u8], CompressionMethod)> = Vec::new();
        let method = |c| match c {
            Compression::Stored => CompressionMethod::Stored,
            Compression::Deflated => CompressionMethod::Deflated,
        };
        let first = package
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() == lead.name);
        let rest = package
            .parts
            .iter()
            .filter(|(n, _)| n.as_str() != lead.name);
        entries.extend(first.map(|(n, b)| (n.as_str(), b.as_slice(), method(lead.compression))));
        entries.extend(rest.map(|(n, b)| (n.as_str(), b.as_slice(), CompressionMethod::Deflated)));
        assert_eq!(
            package.write(lead).expect("writes"),
            crate_archive(&entries, false)
        );
    }
}

#[test]
fn archives_the_zip_crate_writes_read_the_same_including_data_descriptors() {
    let text = "<w:t>r\u{e9}sum\u{e9}</w:t>".repeat(300);
    let entries: [(&str, &[u8], CompressionMethod); 5] = [
        (
            "[Content_Types].xml",
            b"<Types/>",
            CompressionMethod::Deflated,
        ),
        ("word/", b"", CompressionMethod::Stored),
        (
            "word/document.xml",
            text.as_bytes(),
            CompressionMethod::Deflated,
        ),
        ("/leading/slash.xml", b"<x/>", CompressionMethod::Stored),
        ("na\u{ef}ve.xml", b"", CompressionMethod::Deflated),
    ];
    for stream in [false, true] {
        let bytes = crate_archive(&entries, stream);
        let read = ours(&bytes).expect("reads");
        assert_eq!(read, oracle(&bytes), "stream: {stream}");
        assert_eq!(read.len(), 4, "the directory is not a part");
    }
}

#[test]
fn foreign_packages_read_as_the_zip_crate_reads_them() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut seen = 0;
    for dir in ["cdoc-docx/tests/fixtures", "cdoc-odt/tests/fixtures"] {
        for file in std::fs::read_dir(root.join(dir)).expect("fixtures") {
            let path = file.expect("entry").path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if !matches!(ext, "docx" | "docm" | "dotx" | "odt") {
                continue;
            }
            let bytes = std::fs::read(&path).expect("readable");
            assert_eq!(
                ours(&bytes).expect("reads"),
                oracle(&bytes),
                "{}",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen >= 8, "only {seen} foreign packages found");
}

#[test]
fn data_before_the_archive_is_skipped_as_the_zip_crate_skips_it() {
    let mut bytes = b"#!/bin/self-extracting stub\n".to_vec();
    bytes.extend(sample().write(OPC).expect("writes"));
    assert_eq!(ours(&bytes).expect("reads"), sample().parts);
    assert_eq!(oracle(&bytes), sample().parts);
}

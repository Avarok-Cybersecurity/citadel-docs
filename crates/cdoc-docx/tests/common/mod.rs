//! Shared test support: a media port with real behaviour (not a mock) and the round trip.

#![allow(dead_code)]

use cdoc_docx::{read_docx, write_docx, MemoryMedia, ReadLimits, Report, WriteOptions};
pub mod canon;

use cdoc_model::Document;
#[allow(unused_imports)]
pub use cdoc_pkg::test_support::{first_diff, library_for, IdFromBytes};

pub fn write(doc: &Document) -> (Vec<u8>, Report) {
    write_docx(doc, &library_for(doc), WriteOptions::document()).expect("write")
}

pub fn read(bytes: &[u8]) -> (Document, Report) {
    let read = read_docx(bytes, &ReadLimits::generous(), &mut IdFromBytes).expect("read");
    (read.document, read.report)
}

/// Model to docx and back.
pub fn round_trip(doc: &Document) -> (Document, Report, Report) {
    let (bytes, written) = write(doc);
    let (back, read) = read(&bytes);
    (back, written, read)
}

/// Read a document some other program wrote, keeping its images in memory.
pub fn read_foreign(bytes: &[u8]) -> (Document, Report, MemoryMedia) {
    let mut media = MemoryMedia::with_prefix("img-");
    let read = read_docx(bytes, &ReadLimits::generous(), &mut media).expect("read");
    (read.document, read.report, media)
}

/// The text of one part of a package, for tests that look at the XML itself.
pub fn part_text(docx: &[u8], name: &str) -> String {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(docx)).expect("a zip");
    let mut file = archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("part {name} present"));
    let mut text = String::new();
    file.read_to_string(&mut text).expect("utf-8 xml");
    text
}

/// The names of every part in a package.
pub fn part_names(docx: &[u8]) -> Vec<String> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(docx)).expect("a zip");
    archive.file_names().map(str::to_owned).collect()
}

/// A package holding exactly the given parts (name, text), deflated.
pub fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    let bytes: Vec<(&str, &[u8])> = parts.iter().map(|(n, t)| (*n, t.as_bytes())).collect();
    package_bytes(&bytes)
}

/// A package holding exactly the given parts (name, bytes), deflated.
pub fn package_bytes(parts: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(*name, options).expect("start");
        writer.write_all(bytes).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

/// Every part of a package, by name.
pub fn parts(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).expect("an entry");
            let mut data = Vec::new();
            file.read_to_end(&mut data).expect("inflates");
            (file.name().to_owned(), data)
        })
        .collect()
}

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A minimal Word-style package around `body` (the content of `w:body`), plus any extra parts
/// and relationships (as `Relationship` elements) the test needs.
pub fn word_package(body: &str, extra_rels: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let types = String::from(
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Default Extension=\"png\" ContentType=\"image/png\"/>\
         <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>"
    );
    let root_rels = format!(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{R}/officeDocument\" Target=\"word/document.xml\"/></Relationships>"
    );
    let rels = format!(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{extra_rels}</Relationships>"
    );
    let document = format!(
        "<w:document xmlns:w=\"{W}\" xmlns:r=\"{R}\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <w:body>{body}</w:body></w:document>"
    );
    let mut parts: Vec<(&str, &str)> = vec![
        ("[Content_Types].xml", &types),
        ("_rels/.rels", &root_rels),
        ("word/_rels/document.xml.rels", &rels),
        ("word/document.xml", &document),
    ];
    parts.extend_from_slice(extra);
    package(&parts)
}

//! Shared test support: a media port with real behaviour (not a mock) and the round trip.

#![allow(dead_code)]

use cdoc_docx::{read_docx, write_docx, MediaSink, MemoryMedia, ReadLimits, Report};
pub mod canon;

use cdoc_model::{ContentId, Document, ImageBytes, ImageError, ImageLibrary};

/// An image's bytes in these tests: a PNG signature followed by its content id, so the id can
/// always be recovered from the bytes. `library_for` serves them (the workspace content store,
/// which real callers wire in); `IdFromBytes` is the reading side.
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Every image the document names, served as signature plus id.
pub fn library_for(doc: &Document) -> ImageLibrary {
    let mut library = ImageLibrary::empty();
    for inline in doc.inlines() {
        if let cdoc_model::InlineContent::Image(img) = &inline.content {
            let mut bytes = SIGNATURE.to_vec();
            bytes.extend_from_slice(img.content_id.as_str().as_bytes());
            library.add(img.content_id.clone(), "image/png", bytes);
        }
    }
    library
}

/// A stateless sink: the id is whatever the bytes say.
pub struct IdFromBytes;

impl MediaSink for IdFromBytes {
    fn store(&mut self, image: ImageBytes<'_>) -> Result<ContentId, ImageError> {
        let rest = image
            .bytes
            .strip_prefix(SIGNATURE)
            .ok_or_else(|| ImageError("not a test image".to_owned()))?;
        String::from_utf8(rest.to_vec())
            .map(ContentId)
            .map_err(|e| ImageError(e.to_string()))
    }
}

pub fn write(doc: &Document) -> (Vec<u8>, Report) {
    write_docx(doc, &library_for(doc)).expect("write")
}

pub fn read(bytes: &[u8]) -> (Document, Report) {
    read_docx(bytes, &ReadLimits::generous(), &mut IdFromBytes).expect("read")
}

/// Model to docx and back.
pub fn round_trip(doc: &Document) -> (Document, Report, Report) {
    let (bytes, written) = write(doc);
    let (back, read) = read(&bytes);
    (back, written, read)
}

fn diff_values(path: &str, left: &serde_json::Value, right: &serde_json::Value) -> Option<String> {
    use serde_json::Value::{Array, Object};
    match (left, right) {
        (Object(lo), Object(ro)) => {
            let keys: std::collections::BTreeSet<_> = lo.keys().chain(ro.keys()).collect();
            keys.into_iter()
                .find_map(|key| match (lo.get(key), ro.get(key)) {
                    (Some(lv), Some(rv)) => diff_values(&format!("{path}.{key}"), lv, rv),
                    (lv, rv) => Some(format!("{path}.{key}: {lv:?} != {rv:?}")),
                })
        }
        (Array(la), Array(ra)) => {
            if la.len() != ra.len() {
                return Some(format!("{path}: length {} != {}", la.len(), ra.len()));
            }
            la.iter()
                .zip(ra)
                .enumerate()
                .find_map(|(i, (lv, rv))| diff_values(&format!("{path}[{i}]"), lv, rv))
        }
        _ => (left != right).then(|| format!("{path}: {left} != {right}")),
    }
}

/// The first place two documents differ, as a JSON path with both values; `None` when equal.
pub fn first_diff(left: &Document, right: &Document) -> Option<String> {
    let l = serde_json::to_value(left).expect("model serialises");
    let r = serde_json::to_value(right).expect("model serialises");
    diff_values("doc", &l, &r)
}

/// Read a document some other program wrote, keeping its images in memory.
pub fn read_foreign(bytes: &[u8]) -> (Document, Report, MemoryMedia) {
    let mut media = MemoryMedia::with_prefix("img-");
    let (doc, report) = read_docx(bytes, &ReadLimits::generous(), &mut media).expect("read");
    (doc, report, media)
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
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in parts {
        writer.start_file(*name, options).expect("start");
        writer.write_all(text.as_bytes()).expect("write");
    }
    writer.finish().expect("finish").into_inner()
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

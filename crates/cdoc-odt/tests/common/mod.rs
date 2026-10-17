//! Shared test support: a media port with real behaviour (not a mock), the round trip, and
//! package inspection.

#![allow(dead_code)]

pub mod canon;

use cdoc_model::Document;
use cdoc_odt::{read_odt, write_odt, Imported, MemoryMedia, ReadLimits, Report, WriteOptions};
#[allow(unused_imports)]
pub use cdoc_pkg::test_support::{first_diff, library_for, IdFromBytes};

/// The sentence the writer reports for each value kept only in the sidecar.
pub const NO_SLOT: &str = "has no ODF slot; kept in the sidecar";

pub fn write_with(doc: &Document, options: WriteOptions) -> (Vec<u8>, Report) {
    write_odt(doc, &library_for(doc), options).expect("write")
}

pub fn write(doc: &Document) -> (Vec<u8>, Report) {
    write_with(doc, WriteOptions::document())
}

pub fn read_imported(bytes: &[u8]) -> Imported {
    read_odt(bytes, &ReadLimits::generous(), &mut IdFromBytes).expect("read")
}

pub fn read(bytes: &[u8]) -> (Document, Report) {
    let read = read_imported(bytes);
    (read.document, read.report)
}

/// Model to odt and back.
pub fn round_trip(doc: &Document) -> (Document, Report, Report) {
    let (bytes, written) = write(doc);
    let (back, read) = read(&bytes);
    (back, written, read)
}

/// Read a document some other program wrote, keeping its images in memory.
pub fn read_foreign(bytes: &[u8]) -> (Document, Report, MemoryMedia) {
    let mut media = MemoryMedia::with_prefix("img-");
    let read = read_odt(bytes, &ReadLimits::generous(), &mut media).expect("read");
    (read.document, read.report, media)
}

/// The write losses that are not values parked in the sidecar.
pub fn not_parked(report: &Report) -> Vec<String> {
    report
        .losses
        .iter()
        .filter_map(|l| l.what().map(str::to_owned))
        .filter(|w| !w.ends_with(NO_SLOT))
        .collect()
}

/// Every entry of a package in archive order: name, whether it is stored, bytes.
pub fn entries(bytes: &[u8]) -> Vec<(String, bool, Vec<u8>)> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).expect("an entry");
            let stored = file.compression() == zip::CompressionMethod::Stored;
            let mut data = Vec::new();
            file.read_to_end(&mut data).expect("inflates");
            (file.name().to_owned(), stored, data)
        })
        .collect()
}

/// One part's text.
pub fn part_text(bytes: &[u8], name: &str) -> String {
    let found = entries(bytes).into_iter().find(|(n, _, _)| n == name);
    String::from_utf8(found.unwrap_or_else(|| panic!("part {name} present")).2).expect("utf-8")
}

/// A package holding the given parts, `mimetype` first and stored when present.
pub fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in parts {
        let method = if *name == "mimetype" {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        let options = zip::write::SimpleFileOptions::default().compression_method(method);
        writer.start_file(*name, options).expect("start");
        writer.write_all(text.as_bytes()).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

const ROOT: &str = "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
    xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
    xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
    xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
    xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" \
    xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" \
    xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
    xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\" office:version=\"1.2\"";

/// A minimal hand-written text document: `autos` inside `office:automatic-styles` of
/// content.xml, `body` inside `office:text`, `styles` inside `office:styles`.
pub fn odt(autos: &str, body: &str, styles: &str) -> Vec<u8> {
    let content = format!(
        "<office:document-content {ROOT}><office:automatic-styles>{autos}</office:automatic-styles>\
         <office:body><office:text>{body}</office:text></office:body></office:document-content>"
    );
    let styles = format!(
        "<office:document-styles {ROOT}><office:styles>\
         <style:default-style style:family=\"paragraph\"><style:paragraph-properties style:tab-stop-distance=\"36pt\"/>\
         <style:text-properties style:font-name=\"Arial\" fo:font-size=\"11pt\"/></style:default-style>{styles}</office:styles>\
         <office:automatic-styles><style:page-layout style:name=\"pm1\"><style:page-layout-properties \
         fo:page-width=\"595.3pt\" fo:page-height=\"841.9pt\" fo:margin-top=\"72pt\" fo:margin-bottom=\"72pt\" \
         fo:margin-left=\"72pt\" fo:margin-right=\"72pt\"/></style:page-layout></office:automatic-styles>\
         <office:master-styles><style:master-page style:name=\"Standard\" style:page-layout-name=\"pm1\"/>\
         </office:master-styles></office:document-styles>"
    );
    let manifest = "<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.2\">\
        <manifest:file-entry manifest:full-path=\"/\" manifest:media-type=\"application/vnd.oasis.opendocument.text\"/>\
        <manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>\
        <manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>";
    package(&[
        ("mimetype", "application/vnd.oasis.opendocument.text"),
        ("META-INF/manifest.xml", manifest),
        ("content.xml", &content),
        ("styles.xml", &styles),
    ])
}

//! The package the writer produces is a valid ODF 1.2 package: `mimetype` first and stored,
//! every part listed in the manifest with its media type, every XML part well-formed; a
//! template says so in both places, and the reader reports the kind it read.

mod common;

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_odt::{Kind, WriteOptions};
use cdoc_pkg::xml::{parse, Whitespace};

const TEXT: &str = "application/vnd.oasis.opendocument.text";
const TEMPLATE: &str = "application/vnd.oasis.opendocument.text-template";

#[test]
fn mimetype_is_the_first_entry_and_stored_uncompressed() {
    let (bytes, _) = common::write(&kitchen_sink());
    let entries = common::entries(&bytes);
    let (name, stored, data) = &entries[0];
    assert_eq!(name, "mimetype");
    assert!(stored);
    assert_eq!(data.as_slice(), TEXT.as_bytes());
    // ODF 1.2 part 3, 3.3: readers may sniff the type at a fixed offset.
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..38 + TEXT.len()], TEXT.as_bytes());
}

#[test]
fn the_manifest_lists_every_part_and_nothing_else() {
    let (bytes, _) = common::write(&kitchen_sink());
    let manifest = common::part_text(&bytes, "META-INF/manifest.xml");
    let root = parse("manifest", manifest.as_bytes(), Whitespace::Keep).expect("well-formed");
    let mut listed: Vec<String> = root
        .elems()
        .map(|e| {
            assert!(
                e.get("manifest:media-type").is_some_and(|m| !m.is_empty()),
                "{e:?}"
            );
            e.get("manifest:full-path").expect("path").to_owned()
        })
        .collect();
    assert!(root
        .elems()
        .any(|e| e.get("manifest:full-path") == Some("/")
            && e.get("manifest:media-type") == Some(TEXT)));
    let mut parts: Vec<String> = common::entries(&bytes)
        .into_iter()
        .map(|(n, _, _)| n)
        .filter(|n| n != "mimetype" && n != "META-INF/manifest.xml")
        .collect();
    parts.push("/".to_owned());
    listed.sort();
    parts.sort();
    assert_eq!(listed, parts);
    assert!(
        parts.iter().any(|p| p.starts_with("Pictures/")),
        "the kitchen sink has an image"
    );
}

#[test]
fn every_xml_part_is_well_formed() {
    let (bytes, _) = common::write(&kitchen_sink());
    let xml: Vec<_> = common::entries(&bytes)
        .into_iter()
        .filter(|(n, _, _)| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
        })
        .collect();
    assert_eq!(xml.len(), 5, "manifest, content, styles, meta, sidecar");
    for (name, _, data) in xml {
        parse(&name, &data, Whitespace::Keep).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn a_template_is_written_and_read_as_a_template() {
    let doc = kitchen_sink();
    let (bytes, _) = common::write_with(&doc, WriteOptions::template());
    assert_eq!(common::entries(&bytes)[0].2.as_slice(), TEMPLATE.as_bytes());
    assert!(common::part_text(&bytes, "META-INF/manifest.xml").contains(TEMPLATE));
    let read = common::read_imported(&bytes);
    assert_eq!(read.kind, Kind::Template);
    assert_eq!(read.document, doc);
    let (bytes, _) = common::write(&doc);
    assert_eq!(common::read_imported(&bytes).kind, Kind::Document);
}

#[test]
fn a_package_of_another_kind_is_refused() {
    let bytes = common::package(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
        ("content.xml", "<a/>"),
    ]);
    let err = cdoc_odt::read_odt(
        &bytes,
        &cdoc_odt::ReadLimits::generous(),
        &mut common::IdFromBytes,
    )
    .expect_err("refused");
    assert!(err.to_string().contains("spreadsheet"), "{err}");
}

#[test]
fn an_encrypted_package_is_refused() {
    let manifest =
        "<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\">\
        <manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\">\
        <manifest:encryption-data/></manifest:file-entry></manifest:manifest>";
    let bytes = common::package(&[
        ("mimetype", TEXT),
        ("META-INF/manifest.xml", manifest),
        ("content.xml", "x"),
    ]);
    let err = cdoc_odt::read_odt(
        &bytes,
        &cdoc_odt::ReadLimits::generous(),
        &mut common::IdFromBytes,
    )
    .expect_err("refused");
    assert!(err.to_string().contains("encrypted"), "{err}");
}

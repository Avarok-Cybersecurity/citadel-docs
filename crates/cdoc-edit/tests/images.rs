//! Adding images: PNG and JPEG recognised by their signature and sized from their header,
//! other types refused by name, the caller's size limit enforced, and ids derived from the
//! bytes so every peer names one file alike.

mod common;

use cdoc_edit::{Command, EditError};
use cdoc_model::image_probe::{ImageKind, ProbeError};
use cdoc_model::{ImagePort, Length};
use common::{check, editor, paragraphs};

/// A PNG signature and an `IHDR` of `width` × `height` (the header is all the editor reads).
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    b.extend(width.to_be_bytes());
    b.extend(height.to_be_bytes());
    b.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

/// SOI, an SOF0 of 4 × 3 with three components, EOI.
fn jpeg() -> Vec<u8> {
    let mut b = vec![
        0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x03, 0x00, 0x04, 0x03,
    ];
    b.extend([0; 9]);
    b.extend([0xFF, 0xD9]);
    b
}

const LIMIT: usize = 1 << 20;

#[test]
fn a_png_and_a_jpeg_are_held_by_their_content() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let added = e.add_image(png(640, 480), LIMIT).unwrap();
    assert_eq!(
        (added.kind, added.width, added.height),
        (ImageKind::Png, 640, 480)
    );
    let id = added.content_id.as_str();
    assert!(id.starts_with("sha256-") && id.len() == 7 + 64, "{id}");
    let held = e.images().image(&added.content_id).unwrap();
    assert_eq!((held.mime, held.bytes), ("image/png", &png(640, 480)[..]));
    assert_eq!(
        e.add_image(png(640, 480), LIMIT).unwrap().content_id,
        added.content_id,
        "the same file, the same id"
    );
    assert_ne!(
        e.add_image(png(640, 481), LIMIT).unwrap().content_id,
        added.content_id
    );
    let j = e.add_image(jpeg(), LIMIT).unwrap();
    assert_eq!((j.kind, j.width, j.height), (ImageKind::Jpeg, 4, 3));
    e.apply(Command::InsertImage {
        content_id: added.content_id,
        width: Length(7200),
        height: Length(5400),
    })
    .unwrap();
    check(&e);
}

#[test]
fn other_types_are_refused_by_name() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let webp = [b"RIFF\0\0\0\0WEBPVP8 ".as_slice(), &[0; 8]].concat();
    for (bytes, name) in [
        (b"GIF89a\x01\0\x01\0".to_vec(), "GIF"),
        (webp, "WebP"),
        (b"BM\0\0".to_vec(), "BMP"),
        (b"<svg xmlns=''/>".to_vec(), "SVG"),
        (b"hello".to_vec(), "unknown"),
    ] {
        assert_eq!(
            e.add_image(bytes, LIMIT),
            Err(EditError::Image(ProbeError::Unsupported(name)))
        );
    }
    let mut cut = png(10, 10);
    cut.truncate(20);
    assert_eq!(
        e.add_image(cut, LIMIT),
        Err(EditError::Image(ProbeError::Malformed(ImageKind::Png)))
    );
    assert_eq!(
        e.add_image(png(0, 10), LIMIT),
        Err(EditError::Image(ProbeError::Malformed(ImageKind::Png)))
    );
    let mut open = jpeg();
    open.pop();
    assert_eq!(
        e.add_image(open, LIMIT),
        Err(EditError::Image(ProbeError::Malformed(ImageKind::Jpeg)))
    );
}

#[test]
fn the_size_limit_is_the_callers() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let file = png(1, 1);
    let n = file.len();
    assert_eq!(
        e.add_image(file.clone(), n - 1),
        Err(EditError::ImageTooLarge {
            size: n,
            limit: n - 1
        })
    );
    assert!(e.add_image(file, n).is_ok(), "the limit is inclusive");
}

/// The id is SHA-256 of the file, as any content store computes it (digest from Python's
/// `hashlib`).
#[test]
fn the_id_is_the_sha256_of_the_file() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let id = e.add_image(png(1, 1), LIMIT).unwrap().content_id;
    assert_eq!(
        id.as_str(),
        "sha256-7a62e481c41f555352930feb61d703eb7b6225a82815d9aadf3dcc1e5f34b2ac"
    );
}

#[test]
fn the_added_image_record_round_trips_and_refuses_truncation() {
    use cdoc_edit::wire::{decode_added_image, encode_added_image};
    let mut e = editor(&paragraphs(&["x"]), 1);
    for file in [png(70_000, 3), jpeg()] {
        let added = e.add_image(file, LIMIT).unwrap();
        let bytes = encode_added_image(&added);
        assert_eq!(&bytes[..4], b"CDI\x01");
        assert_eq!(decode_added_image(&bytes).unwrap(), added);
        for k in 0..bytes.len() {
            assert!(decode_added_image(&bytes[..k]).is_err(), "cut at {k}");
        }
        assert!(decode_added_image(&[&bytes[..], &[0]].concat()).is_err());
        let mut bad = bytes.clone();
        bad[4 + 1 + 71] = 2;
        assert!(decode_added_image(&bad).is_err(), "kind 2 is not a kind");
    }
}

/// An imported document's images are renamed to their content ids, in the document and in
/// the editor's images; ids the import did not bring stay as the content store knows them.
#[test]
fn an_imported_document_holds_its_images_by_content() {
    use cdoc_model::test_support::fixtures::kitchen_sink;
    let doc = kitchen_sink();
    let mut image_ids = Vec::new();
    let collect = |d: &cdoc_model::Document, out: &mut Vec<String>| {
        let mut d = d.clone();
        d.for_each_paragraph_mut(&mut |p| {
            for i in &p.inlines {
                if let cdoc_model::InlineContent::Image(img) = &i.content {
                    out.push(img.content_id.as_str().to_owned());
                }
            }
        });
    };
    collect(&doc, &mut image_ids);
    assert_eq!(image_ids, ["img-1"]);
    let file = png(3, 2);
    let e = cdoc_edit::Editor::from_imported(
        doc.clone(),
        vec![(cdoc_model::ContentId::new("img-1"), file.clone())],
        LIMIT,
        cdoc_crdt::ClientId(1),
        common::fonts(),
        common::options(),
    )
    .unwrap();
    let mut renamed = Vec::new();
    collect(e.model(), &mut renamed);
    let want = editor(&paragraphs(&["x"]), 2)
        .add_image(file.clone(), LIMIT)
        .unwrap();
    assert_eq!(renamed, [want.content_id.as_str()]);
    assert_eq!(e.images().image(&want.content_id).unwrap().bytes, &file[..]);
    check(&e);
    let kept = cdoc_edit::Editor::from_imported(
        doc.clone(),
        Vec::new(),
        LIMIT,
        cdoc_crdt::ClientId(1),
        common::fonts(),
        common::options(),
    )
    .unwrap();
    let mut ids = Vec::new();
    collect(kept.model(), &mut ids);
    assert_eq!(ids, ["img-1"]);
    let refused = cdoc_edit::Editor::from_imported(
        doc,
        vec![(
            cdoc_model::ContentId::new("img-1"),
            b"GIF89a\x01\0\x01\0".to_vec(),
        )],
        LIMIT,
        cdoc_crdt::ClientId(1),
        common::fonts(),
        common::options(),
    );
    assert!(matches!(
        refused,
        Err(EditError::Image(ProbeError::Unsupported("GIF")))
    ));
}

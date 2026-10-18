//! The document's own styles for the gallery, resolved through their chains; and images
//! inserted without a size, at their own size and no wider than the column.

mod common;

use cdoc_edit::wire::{decode_request, decode_styles, encode_request, encode_styles, Request};
use cdoc_edit::{Command, EditError};
use cdoc_model::{ContentId, FontSize, Length, StyleId, StyleKind};
use common::{check, editor, paragraphs};

#[test]
fn the_styles_record_lists_the_document_s_styles_resolved() {
    let e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let styles = e.styles().unwrap();
    let sheet = &e.model().styles.styles;
    assert_eq!(
        styles.iter().map(|s| &s.id).collect::<Vec<_>>(),
        sheet.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    let h1 = styles
        .iter()
        .find(|s| s.id == StyleId::new("Heading1"))
        .unwrap();
    assert_eq!(h1.kind, StyleKind::Paragraph);
    assert_eq!(h1.based_on, Some(StyleId::new("Normal")));
    assert_eq!(h1.run.size, Some(FontSize::from_points(16)));
    let link = styles
        .iter()
        .find(|s| s.id == StyleId::new("Hyperlink"))
        .unwrap();
    assert_eq!(link.kind, StyleKind::Character);
    assert_eq!(
        (link.run.size, link.run.font.as_deref()),
        (Some(FontSize::from_points(11)), Some("Calibri")),
        "resolved through the defaults"
    );
    assert!(link.run.color.is_some() && link.run.underline.is_some());
    let bytes = encode_styles(&styles);
    assert_eq!(&bytes[..4], b"CDS\x01");
    assert_eq!(decode_styles(&bytes).unwrap(), styles);
    for k in (0..bytes.len()).step_by(7) {
        assert!(decode_styles(&bytes[..k]).is_err(), "cut at {k}");
    }
    assert!(decode_styles(&[&bytes[..], &[0]].concat()).is_err());
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    b.extend(width.to_be_bytes());
    b.extend(height.to_be_bytes());
    b.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

fn inserted(e: &cdoc_edit::Editor<'_>) -> (Length, Length) {
    let cdoc_model::Block::Paragraph(p) = &e.model().sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines
        .iter()
        .find_map(|i| match &i.content {
            cdoc_model::InlineContent::Image(img) => Some((img.width, img.height)),
            _ => None,
        })
        .expect("an image")
}

#[test]
fn an_image_without_a_size_is_its_own_size_up_to_the_column() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let page = e.model().sections[0].properties.page.clone();
    let column = page.size.width.0 - page.margins.left.0 - page.margins.right.0;
    let small = e.add_image(png(100, 50), 1 << 20).unwrap().content_id;
    e.apply(Command::InsertImage {
        content_id: small,
        size: None,
    })
    .unwrap();
    assert_eq!(
        inserted(&e),
        (Length(7500), Length(3750)),
        "0.75 pt a pixel"
    );
    e.undo().unwrap();
    let wide = e.add_image(png(4000, 1000), 1 << 20).unwrap().content_id;
    e.apply(Command::InsertImage {
        content_id: wide,
        size: None,
    })
    .unwrap();
    assert_eq!(inserted(&e), (Length(column), Length(column / 4)));
    check(&e);
    assert!(matches!(
        e.apply(Command::InsertImage {
            content_id: ContentId::new("not-held"),
            size: None,
        }),
        Err(EditError::NotApplicable(_))
    ));
    let req = Request::Command(Command::InsertImage {
        content_id: ContentId::new("a"),
        size: None,
    });
    let bytes = encode_request(&req);
    assert_eq!(bytes[0], 0x26);
    assert_eq!(decode_request(&bytes).unwrap(), req);
}

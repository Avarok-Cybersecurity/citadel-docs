//! Images and signature fields in the written PDF, re-read with lopdf.

mod support;

use lopdf::Object;
use support::{catalog, deref, dict, load, name};

#[test]
fn images_are_xobjects_jpeg_verbatim_and_png_alpha_as_a_soft_mask() {
    let (_, pdf) = support::kitchen();
    let doc = load(&pdf);
    let first = *doc.get_pages().values().next().expect("page");
    let (resources, _) = doc.get_page_resources(first).expect("resources");
    let xobjects = dict(
        &doc,
        resources
            .expect("resources")
            .get(b"XObject")
            .expect("xobjects"),
    );
    let (_, image) = xobjects.iter().next().expect("an image");
    let Object::Stream(stream) = deref(&doc, image) else {
        panic!("stream")
    };
    assert_eq!(
        name(stream.dict.get(b"Filter").expect("filter")),
        "DCTDecode"
    );
    assert_eq!(
        stream.content.as_slice(),
        include_bytes!("data/photo.jpg"),
        "the JPEG is verbatim"
    );

    let mut alpha = cdoc_model::test_support::fixtures::kitchen_sink();
    for section in &mut alpha.sections {
        for block in &mut section.blocks {
            let cdoc_model::Block::Paragraph(p) = block else {
                continue;
            };
            for inline in &mut p.inlines {
                if let cdoc_model::InlineContent::Image(img) = &mut inline.content {
                    img.content_id = cdoc_model::ContentId::new("alpha");
                }
            }
        }
    }
    let pdf = support::pdf_of(&support::laid_out(&alpha));
    let doc = load(&pdf);
    let masked = doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|s| s.dict.get(b"SMask").is_ok());
    assert!(masked, "PNG alpha is carried as a soft mask");
}

#[test]
fn the_signature_field_is_an_unsigned_sig_field_at_its_laid_out_rectangle() {
    let (layout, pdf) = support::kitchen();
    let doc = load(&pdf);
    let form = dict(&doc, catalog(&doc).get(b"AcroForm").expect("form"));
    let fields = form
        .get(b"Fields")
        .expect("fields")
        .as_array()
        .expect("array");
    assert_eq!(fields.len(), 1);
    let field = dict(&doc, &fields[0]);
    assert_eq!(name(field.get(b"FT").expect("type")), "Sig");
    assert_eq!(
        field.get(b"T").expect("name").as_str().expect("str"),
        b"sig1"
    );
    let rect: Vec<f32> = field
        .get(b"Rect")
        .expect("rect")
        .as_array()
        .expect("array")
        .iter()
        .map(|o| o.as_float().expect("n"))
        .collect();
    let sig = &layout.signatures[0];
    let h = layout.pages[0].height;
    let expect = [
        sig.rect.x,
        h - sig.rect.y - sig.rect.height,
        sig.rect.x + sig.rect.width,
        h - sig.rect.y,
    ]
    .map(cdoc_layout::ir::to_pt);
    for (got, want) in rect.iter().zip(expect) {
        assert!((got - want).abs() < 0.011, "{rect:?} vs {expect:?}");
    }
}

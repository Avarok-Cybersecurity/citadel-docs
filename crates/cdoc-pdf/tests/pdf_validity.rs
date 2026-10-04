//! The written PDF, re-read with lopdf: pages, embedded subset faces, text, links, images,
//! metadata, outline and signature fields.

mod support;

use lopdf::Object;
use support::{catalog, deref, dict, load, name};

#[test]
fn every_page_is_written_with_its_size() {
    let (layout, pdf) = support::kitchen();
    let doc = load(&pdf);
    let pages = doc.get_pages();
    assert_eq!(pages.len(), layout.pages.len());
    for id in pages.values() {
        let page = doc.get_dictionary(*id).expect("page");
        let media = page
            .get(b"MediaBox")
            .expect("media box")
            .as_array()
            .expect("array");
        let size: Vec<f32> = media.iter().map(|o| o.as_float().unwrap_or(0.0)).collect();
        assert_eq!(size, [0.0, 0.0, 841.9, 595.3]);
    }
}

#[test]
fn every_face_is_an_embedded_subset() {
    let (_, pdf) = support::kitchen();
    let doc = load(&pdf);
    let whole = cdoc_layout::test_support::FACES[0].2.len();
    let mut seen = 0;
    for id in doc.get_pages().values() {
        for font in doc.get_page_fonts(*id).expect("fonts").values() {
            assert_eq!(name(font.get(b"Subtype").expect("subtype")), "Type0");
            let base = name(font.get(b"BaseFont").expect("base font"));
            assert!(
                base.len() > 7 && base.as_bytes()[6] == b'+',
                "subset tag: {base}"
            );
            assert!(base.contains("Carlito"), "{base}");
            let kids = font
                .get(b"DescendantFonts")
                .expect("descendant")
                .as_array()
                .expect("array");
            let cid = dict(&doc, &kids[0]);
            let descriptor = dict(&doc, cid.get(b"FontDescriptor").expect("descriptor"));
            let Object::Stream(file) = deref(&doc, descriptor.get(b"FontFile2").expect("embedded"))
            else {
                panic!("FontFile2 is a stream");
            };
            let bytes = file.decompressed_content().expect("inflates");
            assert!(
                ttf_parser::Face::parse(&bytes, 0).is_ok(),
                "the subset is a font"
            );
            assert!(
                bytes.len() * 4 < whole,
                "{} bytes is not a subset of {whole}",
                bytes.len()
            );
            seen += 1;
        }
    }
    assert!(seen >= 3, "regular, bold and italic faces are used");
}

#[test]
fn text_reads_back_as_laid_out_through_to_unicode() {
    let (layout, pdf) = support::kitchen();
    let doc = load(&pdf);
    for (page, id) in layout.pages.iter().zip(doc.get_pages().values()) {
        let read: Vec<String> = support::pdf_runs(&doc, *id)
            .into_iter()
            .map(|r| r.3)
            .collect();
        let laid: Vec<String> = page.runs().map(|r| r.text.clone()).collect();
        assert_eq!(read, laid);
    }
}

#[test]
fn links_are_uri_annotations_and_only_web_links_survive() {
    let (_, pdf) = support::kitchen();
    let doc = load(&pdf);
    let first = *doc.get_pages().values().next().expect("page");
    let uris: Vec<String> = doc
        .get_page_annotations(first)
        .expect("annotations")
        .iter()
        .filter(|a| a.get(b"Subtype").is_ok_and(|s| name(s) == "Link"))
        .map(|a| {
            let action = dict(&doc, a.get(b"A").expect("action"));
            String::from_utf8_lossy(action.get(b"URI").expect("uri").as_str().expect("str"))
                .into_owned()
        })
        .collect();
    assert!(
        !uris.is_empty() && uris.iter().all(|u| u == "https://avarok.net"),
        "{uris:?}"
    );

    let mut evil = cdoc_model::test_support::fixtures::kitchen_sink();
    for inline in evil
        .sections
        .iter_mut()
        .flat_map(|s| s.blocks.iter_mut())
        .filter_map(|b| match b {
            cdoc_model::Block::Paragraph(p) => Some(p),
            cdoc_model::Block::Table(_) => None,
        })
        .flat_map(|p| p.inlines.iter_mut())
    {
        if let Some(link) = &mut inline.marks.link {
            link.href = "javascript:alert(1)".to_owned();
        }
    }
    let pdf = support::pdf_of(&support::laid_out(&evil));
    assert!(
        !String::from_utf8_lossy(&pdf).contains("javascript"),
        "the script link is dropped"
    );
}

#[test]
fn the_information_dictionary_and_language_come_from_the_document_and_the_clock() {
    let mut doc_model = cdoc_model::test_support::fixtures::kitchen_sink();
    doc_model.meta.language = Some("en-GB".to_owned());
    let pdf = support::pdf_of(&support::laid_out(&doc_model));
    let doc = load(&pdf);
    let info = dict(&doc, doc.trailer.get(b"Info").expect("info"));
    let text = |k: &[u8]| {
        String::from_utf8_lossy(info.get(k).expect("entry").as_str().expect("string")).into_owned()
    };
    assert_eq!(text(b"Title"), "Kitchen sink");
    assert_eq!(text(b"Producer"), "Citadel Docs");
    assert!(
        text(b"CreationDate").starts_with("D:20261004120000"),
        "{}",
        text(b"CreationDate")
    );
    let lang = catalog(&doc)
        .get(b"Lang")
        .expect("lang")
        .as_str()
        .expect("string");
    assert_eq!(lang, b"en-GB");
}

#[test]
fn headings_become_an_outline_pointing_at_their_page() {
    let (layout, pdf) = support::kitchen();
    let doc = load(&pdf);
    let outlines = dict(&doc, catalog(&doc).get(b"Outlines").expect("outlines"));
    let first = dict(&doc, outlines.get(b"First").expect("first"));
    assert_eq!(
        first.get(b"Title").expect("title").as_str().expect("str"),
        b"Every kind"
    );
    let dest = first.get(b"Dest").expect("dest").as_array().expect("array");
    let page = dest[0].as_reference().expect("page ref");
    let index = layout.outline[0].page;
    assert_eq!(Some(&page), doc.get_pages().get(&(index + 1)));
}

#[test]
fn a_bad_clock_and_a_missing_image_are_errors_not_files() {
    let (layout, _) = support::kitchen();
    let bad = cdoc_pdf::render_pdf(
        &layout,
        &support::fonts(),
        &support::images(),
        &cdoc_pdf::FixedClock("now".into()),
    );
    assert!(matches!(bad, Err(cdoc_pdf::PdfError::Clock(_))));
    let none = cdoc_pdf::render_pdf(
        &layout,
        &support::fonts(),
        &cdoc_pdf::ImageLibrary::empty(),
        &cdoc_pdf::FixedClock(support::NOW.into()),
    );
    assert!(matches!(none, Err(cdoc_pdf::PdfError::MissingImage(e)) if e.0 == "img-1"));
}

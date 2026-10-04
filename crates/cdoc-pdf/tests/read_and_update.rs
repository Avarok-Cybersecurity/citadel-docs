//! Reading existing PDFs and appending incremental updates, checked with lopdf (an independent
//! reader) and with this crate's own reader.

mod support;

use cdoc_model::Color;
use cdoc_pdf::{
    append_annotations, Annotation, AnnotationKind, FixedClock, PdfFile, ReadError, UpdateError,
};

fn clock() -> FixedClock {
    FixedClock(support::NOW.to_owned())
}

fn note(page: usize) -> Annotation {
    Annotation {
        page,
        rect: [100.0, 400.0, 120.0, 420.0],
        kind: AnnotationKind::Note {
            author: "Alice".into(),
            contents: "Check this".into(),
        },
    }
}

fn square(page: usize) -> Annotation {
    Annotation {
        page,
        rect: [50.0, 50.0, 200.0, 100.0],
        kind: AnnotationKind::Square {
            colour: Color::rgb(255, 0, 0),
            width: 2.0,
        },
    }
}

fn signature(page: usize) -> Annotation {
    Annotation {
        page,
        rect: [300.0, 60.0, 500.0, 100.0],
        kind: AnnotationKind::Signature {
            name: "witness".into(),
            signer_role: "Witness".into(),
        },
    }
}

fn subtypes(doc: &lopdf::Document, page: u32) -> Vec<String> {
    let id = doc.get_pages()[&page];
    doc.get_page_annotations(id)
        .expect("annots")
        .iter()
        .map(|a| support::name(a.get(b"Subtype").expect("subtype")))
        .collect()
}

/// A modern file (cross-reference stream, object streams) written by lopdf, not by this crate.
fn modern() -> Vec<u8> {
    let (_, pdf) = support::kitchen();
    let mut doc = support::load(&pdf);
    let mut out = Vec::new();
    doc.save_modern(&mut out).expect("lopdf writes");
    assert!(
        String::from_utf8_lossy(&out).contains("/XRef"),
        "a cross-reference stream"
    );
    out
}

#[test]
fn page_count_and_boxes_are_read_from_a_written_file() {
    let (layout, pdf) = support::kitchen();
    let file = PdfFile::parse(&pdf).expect("parses");
    assert_eq!(file.page_count(), layout.pages.len());
    let boxes = file.page_boxes();
    let near = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01);
    assert!(
        boxes.iter().all(|b| near(b.media, [0.0, 0.0, 841.9, 595.3])
            && near(b.crop, b.media)
            && b.rotation == 0),
        "{boxes:?}"
    );
    assert_eq!(
        PdfFile::parse(&modern()).expect("modern").page_count(),
        layout.pages.len()
    );
}

#[test]
fn garbage_is_not_a_pdf() {
    assert_eq!(PdfFile::parse(b"hello").err(), Some(ReadError::Invalid));
}

#[test]
fn an_update_keeps_the_original_as_a_prefix_and_lopdf_sees_the_new_annotations() {
    let (layout, pdf) = support::kitchen();
    let updated =
        append_annotations(&pdf, &[note(0), square(0), signature(1)], &clock()).expect("appends");
    assert!(
        updated.starts_with(&pdf),
        "incremental: nothing before the update changed"
    );
    let doc = support::load(&updated);
    assert_eq!(doc.get_pages().len(), layout.pages.len());
    let first = subtypes(&doc, 1);
    assert!(
        first.contains(&"Link".to_owned()),
        "existing annotations kept: {first:?}"
    );
    assert!(
        first.contains(&"Text".to_owned()) && first.contains(&"Square".to_owned()),
        "{first:?}"
    );
    assert_eq!(subtypes(&doc, 2), ["Widget"]);
    let form = support::dict(&doc, support::catalog(&doc).get(b"AcroForm").expect("form"));
    let names: Vec<Vec<u8>> = form
        .get(b"Fields")
        .expect("fields")
        .as_array()
        .expect("array")
        .iter()
        .map(|f| {
            support::dict(&doc, f)
                .get(b"T")
                .expect("name")
                .as_str()
                .expect("str")
                .to_vec()
        })
        .collect();
    assert_eq!(
        names,
        [b"sig1".to_vec(), b"witness".to_vec()],
        "the original field and the new one"
    );
    assert_eq!(
        PdfFile::parse(&updated).expect("re-reads").page_count(),
        layout.pages.len()
    );
}

#[test]
fn updates_stack_and_a_file_without_a_form_gains_one() {
    let mut plain = cdoc_model::Document::blank();
    plain.sections[0].blocks = vec![cdoc_model::Block::Paragraph(cdoc_model::Paragraph {
        properties: cdoc_model::ParagraphProperties::none(),
        inlines: vec![cdoc_model::Inline::text(
            "plain",
            cdoc_model::RunProperties::none(),
        )],
    })];
    let pdf = support::pdf_of(&support::laid_out(&plain));
    let once = append_annotations(&pdf, &[signature(0)], &clock()).expect("first update");
    let twice = append_annotations(&once, &[note(0)], &clock()).expect("second update");
    assert!(twice.starts_with(&once));
    let doc = support::load(&twice);
    assert_eq!(subtypes(&doc, 1), ["Widget", "Text"]);
    assert!(support::catalog(&doc).get(b"AcroForm").is_ok());
}

#[test]
fn a_file_ending_in_a_cross_reference_stream_gets_a_stream_section() {
    let original = modern();
    let updated = append_annotations(&original, &[note(1)], &clock()).expect("appends");
    let tail = String::from_utf8_lossy(&updated[original.len()..]).into_owned();
    assert!(
        tail.contains("/Type /XRef") && !tail.contains("\nxref\n"),
        "{tail}"
    );
    let doc = support::load(&updated);
    assert!(subtypes(&doc, 2).contains(&"Text".to_owned()));
}

#[test]
fn bad_requests_are_refused_by_name() {
    let (_, pdf) = support::kitchen();
    assert_eq!(
        append_annotations(&pdf, &[note(9)], &clock()),
        Err(UpdateError::NoPage(9))
    );
    assert!(matches!(
        append_annotations(&pdf, &[note(0)], &FixedClock("soon".into())),
        Err(UpdateError::Clock(_))
    ));
    let mut cut = pdf.clone();
    let at = cut
        .windows(9)
        .rposition(|w| w == b"startxref")
        .expect("startxref");
    cut.truncate(at);
    assert!(
        append_annotations(&cut, &[note(0)], &clock()).is_err(),
        "no startxref, no update"
    );
}

#[test]
fn an_encrypted_trailer_is_refused() {
    let (_, pdf) = support::kitchen();
    let at = pdf
        .windows(7)
        .rposition(|w| w == b"trailer")
        .expect("trailer");
    let open = at + pdf[at..].windows(2).position(|w| w == b"<<").expect("dict") + 2;
    let forged = [&pdf[..open], b" /Encrypt 1 0 R ".as_slice(), &pdf[open..]].concat();
    let err = append_annotations(&forged, &[note(0)], &clock()).expect_err("refused");
    assert_eq!(err, UpdateError::Encrypted, "refused before any parsing");
}

#[test]
fn every_added_annotation_carries_a_normal_appearance_for_viewers_that_draw_only_those() {
    let (_, pdf) = support::kitchen();
    let updated =
        append_annotations(&pdf, &[note(0), square(0), signature(0)], &clock()).expect("appends");
    let doc = support::load(&updated);
    let first = doc.get_pages()[&1];
    let added: Vec<_> = doc
        .get_page_annotations(first)
        .expect("annots")
        .into_iter()
        .filter(|a| a.get(b"Subtype").is_ok_and(|s| support::name(s) != "Link"))
        .filter(|a| {
            a.get(b"T")
                .map_or(true, |t| t.as_str().ok() != Some(b"sig1".as_slice()))
        })
        .collect();
    assert_eq!(added.len(), 3);
    for annot in added {
        let ap = support::dict(&doc, annot.get(b"AP").expect("an appearance"));
        let normal = support::deref(&doc, ap.get(b"N").expect("normal"));
        assert!(
            normal.as_stream().is_ok(),
            "the normal appearance is a form stream"
        );
    }
}

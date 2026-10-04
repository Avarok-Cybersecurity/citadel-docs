//! One IR, two drawings: the PDF and the SVG preview must put every run at the same place, in
//! the same size, with the same text.

mod support;

use cdoc_pdf::svg::{escape, pages_svg};

/// `(x, y, size, text)` of every `<text>` element, y down, text unescaped.
fn svg_runs(svg: &str) -> Vec<(f32, f32, f32, String)> {
    let attr = |tag: &str, name: &str| -> f32 {
        let key = format!(" {name}=\"");
        let start = tag.find(&key).expect("attribute") + key.len();
        tag[start..]
            .split('"')
            .next()
            .expect("value")
            .parse()
            .expect("number")
    };
    svg.split("<text ")
        .skip(1)
        .map(|chunk| {
            let (tag, rest) = chunk.split_once('>').expect("tag end");
            let tag = &format!(" {tag}");
            let text = rest.split("</text>").next().expect("text");
            let text = text
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace("&amp;", "&");
            (attr(tag, "x"), attr(tag, "y"), attr(tag, "font-size"), text)
        })
        .collect()
}

#[test]
fn pdf_and_svg_place_every_run_identically() {
    let (layout, pdf) = support::kitchen();
    let doc = support::load(&pdf);
    let svgs = pages_svg(&layout, &support::images()).expect("svg");
    assert_eq!(svgs.len(), layout.pages.len());
    for ((page, svg), id) in layout.pages.iter().zip(&svgs).zip(doc.get_pages().values()) {
        let height = cdoc_layout::ir::to_pt(page.height);
        let from_pdf: Vec<_> = support::pdf_runs(&doc, *id)
            .into_iter()
            .map(|(x, y, s, t)| (x, height - y, s, t))
            .collect();
        let from_svg = svg_runs(svg);
        assert_eq!(from_pdf.len(), from_svg.len());
        assert_ne!(from_pdf.len(), 0);
        for (p, s) in from_pdf.iter().zip(&from_svg) {
            assert!(
                (p.0 - s.0).abs() < 0.006 && (p.1 - s.1).abs() < 0.006,
                "{p:?} vs {s:?}"
            );
            assert!((p.2 - s.2).abs() < 0.006, "size {p:?} vs {s:?}");
            assert_eq!(p.3, s.3);
        }
    }
}

#[test]
fn the_preview_escapes_untrusted_text_and_drops_script_links() {
    let mut doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let first = doc.sections[0].blocks.first_mut().expect("block");
    if let cdoc_model::Block::Paragraph(p) = first {
        p.inlines = vec![cdoc_model::Inline {
            content: cdoc_model::InlineContent::Text(cdoc_model::TextRun {
                text: "<script>alert('x')</script> & \"q\"".to_owned(),
                properties: cdoc_model::RunProperties::none(),
            }),
            marks: cdoc_model::Marks {
                link: Some(cdoc_model::Link {
                    href: "javascript:alert(1)".to_owned(),
                    title: None,
                }),
                ..cdoc_model::Marks::none()
            },
        }];
    }
    let svg = pages_svg(&support::laid_out(&doc), &support::images())
        .expect("svg")
        .remove(0);
    assert!(!svg.contains("<script"), "text is escaped");
    assert!(svg.contains(&escape("<script>alert('x')</script>")));
    assert!(
        !svg.contains("javascript:"),
        "the script link is not an <a>"
    );
    assert!(
        svg.contains("href=\"https://avarok.net\"") || !svg.contains("<a "),
        "only web links"
    );
    assert!(svg.contains("font-family=\"'Carlito'\"") && svg.contains("font-kerning:none"));
}

#[test]
fn preview_images_are_the_same_bytes_as_data_uris() {
    use base64::Engine as _;
    let (layout, _) = support::kitchen();
    let svg = &pages_svg(&layout, &support::images()).expect("svg")[0];
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(include_bytes!("data/photo.jpg"));
    assert!(svg.contains(&format!("href=\"data:image/jpeg;base64,{encoded}\"")));
}

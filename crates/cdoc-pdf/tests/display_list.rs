//! The display list and the PDF place the same glyphs: for every run of the kitchen sink, the
//! face, the size, the baseline origin, the glyph ids and every glyph's drawn position (pen
//! plus offset, and its rise) the display list paints are what the PDF's content stream shows,
//! read back with `lopdf` (an independent reader) through `TJ`, `Ts` and each font's `W`
//! widths. A CID is the glyph id, so no `cmap` is consulted on either side. The PDF writer
//! still draws from the IR, so this is what keeps the two paths one description.

mod support;

use cdoc_paint::{decode, encode, OpView};
use lopdf::Object;
use std::collections::BTreeMap;

/// `(glyph id, drawn x, rise)`, in points; rise is upwards.
type Placed = (u16, f32, f32);

/// `(face id, size, origin, glyphs)` per run, in points, page space (y down).
type Run = (u16, f32, (f32, f32), Vec<Placed>);

fn number(o: &Object) -> f32 {
    o.as_float()
        .or_else(|_| {
            o.as_i64()
                .map(|i| f32::from(i16::try_from(i).expect("small")))
        })
        .expect("number")
}

/// Subset glyph id → width in thousandths of an em, from a Type 0 font's descendant `W`.
fn widths(doc: &lopdf::Document, font: &lopdf::Dictionary) -> BTreeMap<u16, f32> {
    let descendants = support::deref(doc, font.get(b"DescendantFonts").expect("descendants"));
    let cid = support::dict(doc, &descendants.as_array().expect("array")[0]);
    let w = support::deref(doc, cid.get(b"W").expect("W"))
        .as_array()
        .expect("W array");
    let mut out = BTreeMap::new();
    for pair in w.chunks(2) {
        let first = u16::try_from(pair[0].as_i64().expect("first cid")).expect("u16");
        for (k, width) in pair[1].as_array().expect("widths").iter().enumerate() {
            out.insert(first + u16::try_from(k).expect("u16"), number(width));
        }
    }
    out
}

fn pdf_runs(doc: &lopdf::Document, page: lopdf::ObjectId, height: f32) -> Vec<Run> {
    let mut maps = BTreeMap::new();
    for (name, font) in doc.get_page_fonts(page).expect("fonts") {
        let id: u16 = String::from_utf8(name[1..].to_vec())
            .expect("F<n>")
            .parse()
            .expect("n");
        maps.insert(name, (id, widths(doc, font)));
    }
    let content = lopdf::content::Content::decode(&doc.get_page_content(page)).expect("decodes");
    let (mut font, mut size, mut rise) = (Vec::new(), 0.0, 0.0);
    let mut out: Vec<Run> = Vec::new();
    let mut pen = 0.0;
    for op in content.operations {
        match op.operator.as_str() {
            "Tf" => {
                font = op.operands[0].as_name().expect("name").to_vec();
                size = number(&op.operands[1]);
            }
            "Tm" => {
                let at = (number(&op.operands[4]), height - number(&op.operands[5]));
                pen = at.0;
                out.push((maps[&font].0, size, at, Vec::new()));
            }
            "Ts" => rise = number(&op.operands[0]),
            "TJ" => {
                let w = &maps[&font].1;
                let run = out.last_mut().expect("Tm first");
                for item in op.operands[0].as_array().expect("TJ array") {
                    if let Ok(bytes) = item.as_str() {
                        for g in bytes.chunks(2) {
                            let id = u16::from_be_bytes([g[0], g[1]]);
                            run.3.push((id, pen, rise));
                            pen += w[&id] * size / 1000.0;
                        }
                    } else {
                        pen -= number(item) * size / 1000.0;
                    }
                }
            }
            "Tj" => panic!("runs are shown with TJ"),
            _ => {}
        }
    }
    out
}

fn list_runs(page: &cdoc_paint::PageView<'_>) -> Vec<Run> {
    let pt = cdoc_layout::ir::to_pt;
    page.ops()
        .filter_map(|op| match op {
            OpView::GlyphRun { head, glyphs, .. } => {
                let mut pen = head.x;
                let placed = glyphs
                    .map(|g| {
                        let at = (g.id, pt(pen + g.x_offset), -pt(g.y_offset));
                        pen += g.advance;
                        at
                    })
                    .collect();
                Some((head.face.0, pt(head.size), (pt(head.x), pt(head.y)), placed))
            }
            _ => None,
        })
        .collect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.006
}

#[test]
fn the_display_list_paints_the_glyphs_the_pdf_places() {
    let (layout, pdf) = support::kitchen();
    let doc = support::load(&pdf);
    let bytes = encode(&layout, &support::fonts()).expect("encodes");
    let view = decode(&bytes).expect("decodes");
    let pages = doc.get_pages();
    assert_eq!(view.page_count(), pages.len());
    let mut glyphs = 0;
    for (slot, id) in pages.values().enumerate() {
        let page = view.page(slot).expect("page");
        let from_pdf = pdf_runs(&doc, *id, cdoc_layout::ir::to_pt(page.height));
        let from_list = list_runs(&page);
        assert_eq!(from_pdf.len(), from_list.len(), "runs on page {slot}");
        for (p, l) in from_pdf.iter().zip(&from_list) {
            assert_eq!(p.0, l.0, "face");
            assert!(
                close(p.1, l.1) && close(p.2 .0, l.2 .0) && close(p.2 .1, l.2 .1),
                "{p:?} vs {l:?}"
            );
            let ids = |r: &Run| r.3.iter().map(|g| g.0).collect::<Vec<_>>();
            assert_eq!(ids(p), ids(l), "glyph ids");
            for (a, b) in p.3.iter().zip(&l.3) {
                assert!(
                    close(a.1, b.1) && close(a.2, b.2),
                    "glyph {} at {:?} vs {:?}",
                    a.0,
                    (a.1, a.2),
                    (b.1, b.2)
                );
            }
            glyphs += l.3.len();
        }
    }
    assert!(glyphs > 150, "only {glyphs} glyphs compared");
}

#[test]
fn the_preview_is_the_display_list_painted() {
    let (layout, _) = support::kitchen();
    let fonts = support::fonts();
    let bytes = encode(&layout, &fonts).expect("encodes");
    let view = decode(&bytes).expect("decodes");
    let painted = cdoc_paint::svg::render_pages(&view, &support::images()).expect("paints");
    let preview = cdoc_pdf::svg::pages_svg(&layout, &fonts, &support::images()).expect("svg");
    assert_eq!(preview, painted);
}

#[test]
fn the_preview_is_painted_from_the_faces_the_file_embeds() {
    let (layout, _) = support::kitchen();
    let none = cdoc_layout::FontLibrary::new("Nothing");
    let preview = cdoc_pdf::svg::pages_svg(&layout, &none, &support::images());
    assert!(
        matches!(
            preview,
            Err(cdoc_pdf::PdfError::Paint(cdoc_paint::PaintError::Font(_)))
        ),
        "glyphs come from the faces, so no faces is no preview: {preview:?}"
    );
}

/// An image of a type the documents do not draw is a placeholder box in the preview and in
/// the file, never an error and never bytes handed to the browser unchecked.
#[test]
fn an_undrawn_image_type_is_a_placeholder() {
    let (layout, _) = support::kitchen();
    let fonts = support::fonts();
    let mut gif = cdoc_pdf::ImageLibrary::empty();
    gif.add(
        cdoc_model::ContentId::new("img-1"),
        "image/gif",
        b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec(),
    );
    let preview = cdoc_pdf::svg::pages_svg(&layout, &fonts, &gif)
        .expect("svg")
        .concat();
    assert!(!preview.contains("data:image/gif"));
    assert!(preview.contains("fill=\"#eeeeee\" stroke=\"#999999\""));
    let clock = cdoc_pdf::FixedClock("2026-10-18T00:00:00Z".to_owned());
    let pdf = cdoc_pdf::render_pdf(&layout, &fonts, &gif, &clock).expect("a pdf");
    assert!(!support::load(&pdf).objects.values().any(|o| {
        o.as_stream().is_ok_and(|s| {
            s.dict
                .get(b"Subtype")
                .is_ok_and(|t| support::name(t) == "Image")
        })
    }));
    let lost = cdoc_pdf::unrendered(&layout, &gif).expect("reads");
    assert_eq!(
        lost.values().collect::<Vec<_>>(),
        ["image type GIF not rendered"]
    );
}

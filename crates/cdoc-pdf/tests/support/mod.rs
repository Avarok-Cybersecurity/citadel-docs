//! Shared by the PDF tests: the kitchen sink laid out with the shipped faces, its image, a
//! fixed clock, and readers for the written file that go through `lopdf`, an independent PDF
//! implementation, rather than this crate's own reader.

#![allow(dead_code, reason = "each test binary uses a different part")]

use cdoc_layout::{layout, FontLibrary, Layout, LayoutOptions};
use cdoc_model::{ContentId, Document};
use cdoc_pdf::{render_pdf, FixedClock, ImageLibrary};
use lopdf::{Dictionary, Object};
use std::collections::BTreeMap;

pub const NOW: &str = "2026-10-04T12:00:00Z";

pub fn fonts() -> FontLibrary {
    cdoc_layout::test_support::default_fonts()
}

pub fn images() -> ImageLibrary {
    let mut lib = ImageLibrary::empty();
    lib.add(
        ContentId::new("img-1"),
        "image/jpeg",
        include_bytes!("../data/photo.jpg").to_vec(),
    );
    lib.add(
        ContentId::new("alpha"),
        "image/png",
        include_bytes!("../data/alpha.png").to_vec(),
    );
    lib.add(
        ContentId::new("opaque"),
        "image/png",
        include_bytes!("../data/opaque.png").to_vec(),
    );
    lib
}

pub fn laid_out(doc: &Document) -> Layout {
    layout(doc, &fonts(), &LayoutOptions::word_like()).expect("lays out")
}

pub fn pdf_of(layout: &Layout) -> Vec<u8> {
    render_pdf(layout, &fonts(), &images(), &FixedClock(NOW.to_owned())).expect("renders")
}

pub fn kitchen() -> (Layout, Vec<u8>) {
    let layout = laid_out(&cdoc_model::test_support::fixtures::kitchen_sink());
    let pdf = pdf_of(&layout);
    (layout, pdf)
}

pub fn load(pdf: &[u8]) -> lopdf::Document {
    lopdf::Document::load_mem(pdf).expect("lopdf reads the file")
}

pub fn deref<'a>(doc: &'a lopdf::Document, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => doc.get_object(*id).expect("reference resolves"),
        other => other,
    }
}

pub fn dict<'a>(doc: &'a lopdf::Document, object: &'a Object) -> &'a Dictionary {
    match deref(doc, object) {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        other => panic!("not a dictionary: {other:?}"),
    }
}

pub fn name(object: &Object) -> String {
    String::from_utf8_lossy(object.as_name().expect("a name")).into_owned()
}

pub fn catalog(doc: &lopdf::Document) -> &Dictionary {
    dict(doc, doc.trailer.get(b"Root").expect("root"))
}

/// The glyph-id → text map of a Type 0 font's `ToUnicode` map.
pub fn to_unicode(doc: &lopdf::Document, font: &Dictionary) -> BTreeMap<u16, String> {
    let Object::Stream(stream) = deref(doc, font.get(b"ToUnicode").expect("ToUnicode")) else {
        panic!("ToUnicode is a stream");
    };
    let text = String::from_utf8(
        stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone()),
    )
    .expect("ascii cmap");
    let mut map = BTreeMap::new();
    let mut in_chars = false;
    for line in text.lines() {
        let line = line.trim();
        if line.ends_with("beginbfchar") {
            in_chars = true;
        } else if line == "endbfchar" {
            in_chars = false;
        } else if in_chars {
            let hex: Vec<&str> = line
                .split(['<', '>'])
                .filter(|s| !s.trim().is_empty())
                .collect();
            let gid = u16::from_str_radix(hex[0], 16).expect("gid");
            let units: Vec<u16> = (0..hex[1].len() / 4)
                .map(|i| u16::from_str_radix(&hex[1][i * 4..i * 4 + 4], 16).expect("utf16"))
                .collect();
            map.insert(gid, String::from_utf16(&units).expect("utf16"));
        }
    }
    map
}

/// Every text run on a page as `(x, y, size, text)`, in points, y up, decoded through each
/// font's `ToUnicode`.
pub fn pdf_runs(doc: &lopdf::Document, page: lopdf::ObjectId) -> Vec<(f32, f32, f32, String)> {
    let fonts = doc.get_page_fonts(page).expect("fonts");
    let maps: BTreeMap<Vec<u8>, BTreeMap<u16, String>> = fonts
        .iter()
        .map(|(k, f)| (k.clone(), to_unicode(doc, f)))
        .collect();
    let content = lopdf::content::Content::decode(&doc.get_page_content(page)).expect("decodes");
    let mut out = Vec::new();
    let (mut font, mut size, mut at) = (Vec::new(), 0.0, (0.0, 0.0));
    let num = |o: &Object| {
        o.as_float()
            .or_else(|_| {
                o.as_i64()
                    .map(|i| f32::from(i16::try_from(i).expect("small")))
            })
            .expect("number")
    };
    for op in content.operations {
        match op.operator.as_str() {
            "Tf" => {
                font = op.operands[0].as_name().expect("font").to_vec();
                size = num(&op.operands[1]);
            }
            "Tm" => at = (num(&op.operands[4]), num(&op.operands[5])),
            "Tj" => {
                let bytes = op.operands[0].as_str().expect("string");
                let map = &maps[&font];
                let text: String = bytes
                    .chunks(2)
                    .map(|g| map[&u16::from_be_bytes([g[0], g[1]])].clone())
                    .collect();
                out.push((at.0, at.1, size, text));
            }
            _ => {}
        }
    }
    out
}

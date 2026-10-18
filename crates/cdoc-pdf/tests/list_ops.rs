//! What the PDF draws from display-list operations the layout does not emit yet (stroked
//! rectangles, clips, translucent colours) and what it refuses: a partial list, and a
//! heading on a page the list lacks. Read back with `lopdf`.

mod support;

use cdoc_layout::{OutlineEntry, Rect};
use cdoc_paint::{DisplayList, DocumentInfo, Op, PageOps, Rgba};
use cdoc_pdf::{render_display_list, FixedClock, PdfError};

const BOX: Rect = Rect {
    x: 7200,
    y: 7200,
    width: 3600,
    height: 1800,
};

fn list(ops: Vec<Op>, number: u32, outline: Vec<OutlineEntry>) -> Vec<u8> {
    DisplayList {
        faces: Vec::new(),
        document: DocumentInfo {
            title: Some("Ops".to_owned()),
            language: Some("en-GB".to_owned()),
            outline,
        },
        pages: vec![PageOps {
            number,
            width: 59_528,
            height: 84_188,
            ops,
        }],
    }
    .to_bytes()
    .expect("writes")
}

fn render(bytes: &[u8]) -> Result<Vec<u8>, PdfError> {
    render_display_list(
        bytes,
        &support::fonts(),
        &support::images(),
        &FixedClock(support::NOW.to_owned()),
    )
}

fn operators(doc: &lopdf::Document) -> Vec<String> {
    let page = *doc.get_pages().values().next().expect("a page");
    let content = lopdf::content::Content::decode(&doc.get_page_content(page)).expect("content");
    content.operations.into_iter().map(|o| o.operator).collect()
}

#[test]
fn stroked_rectangles_clips_and_translucent_colours_are_drawn() {
    let half = Rgba {
        r: 200,
        g: 0,
        b: 0,
        a: 128,
    };
    let ops = vec![
        Op::ClipPush(BOX),
        Op::StrokeRect {
            rect: BOX,
            width: 100,
            colour: half,
        },
        Op::ClipPop,
    ];
    let pdf = render(&list(ops, 0, Vec::new())).expect("renders");
    let doc = support::load(&pdf);
    let ops = operators(&doc).join(" ");
    assert!(ops.contains("q re W n q gs RG w j re S Q Q"), "{ops}");
    let page = *doc.get_pages().values().next().expect("a page");
    let page = doc.get_dictionary(page).expect("the page");
    let resources = support::dict(&doc, page.get(b"Resources").expect("resources"));
    let states = support::dict(&doc, resources.get(b"ExtGState").expect("ExtGState"));
    let state = support::dict(&doc, states.get(b"GA128").expect("the alpha's state"));
    let alpha = state
        .get(b"ca")
        .and_then(lopdf::Object::as_float)
        .expect("/ca");
    assert!((alpha - 128.0 / 255.0).abs() < 1e-3, "{alpha}");
}

#[test]
fn an_opaque_page_has_no_graphics_states() {
    let ops = vec![Op::FillRect {
        rect: BOX,
        colour: Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
    }];
    let pdf = render(&list(ops, 0, Vec::new())).expect("renders");
    let doc = support::load(&pdf);
    assert!(!operators(&doc).iter().any(|o| o == "gs"));
}

#[test]
fn a_partial_list_and_a_heading_off_the_list_are_refused_by_name() {
    assert_eq!(
        render(&list(Vec::new(), 3, Vec::new())).err(),
        Some(PdfError::PartialList(3))
    );
    let lost = OutlineEntry {
        level: 0,
        title: "Gone".to_owned(),
        page: 1,
        y: 0,
    };
    assert_eq!(
        render(&list(Vec::new(), 0, vec![lost])).err(),
        Some(PdfError::NoOutlinePage(1))
    );
}

//! The model's kitchen sink, laid out: a golden snapshot of every run, plus the properties the
//! snapshot alone would not make obvious.

mod support;

use cdoc_layout::{list_labels, write_anchors, Draw};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{Block, InlineContent};

const GOLDEN: &str = include_str!("golden/kitchen_sink.txt");

#[test]
fn the_kitchen_sink_lays_out_exactly_as_recorded() {
    let dump = support::dump(&support::laid_out(&kitchen_sink()));
    if std::env::var_os("CDOC_BLESS").is_some() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/kitchen_sink.txt");
        std::fs::write(path, &dump).expect("writes the golden file");
    }
    assert_eq!(
        dump, GOLDEN,
        "rerun with CDOC_BLESS=1 after an intended change"
    );
}

#[test]
fn pages_are_the_landscape_a4_the_section_asks_for() {
    let layout = support::laid_out(&kitchen_sink());
    assert_eq!(layout.pages.len(), 3);
    for page in &layout.pages {
        assert_eq!((page.width, page.height), (84190, 59530));
    }
}

#[test]
fn headers_follow_title_page_and_even_page_rules_with_live_page_numbers() {
    let layout = support::laid_out(&kitchen_sink());
    let text: Vec<String> = layout.pages.iter().map(cdoc_layout::Page::text).collect();
    assert!(text[0].contains("first header1"), "{}", text[0]);
    assert!(text[1].contains("even header2"), "{}", text[1]);
    assert!(
        text[2].contains("first header3"),
        "the second section's own first page"
    );
}

#[test]
fn the_footnote_is_set_below_the_body_on_the_page_that_references_it() {
    let layout = support::laid_out(&kitchen_sink());
    let page = &layout.pages[0];
    let note = page
        .runs()
        .find(|r| r.text == "A note")
        .expect("the note is on page 0");
    let body_bottom = page
        .runs()
        .filter(|r| r.text != "A note" && r.text != "1" && r.y > 6000)
        .map(|r| r.y)
        .max()
        .expect("body text");
    assert!(
        note.y > body_bottom,
        "note {} above body {}",
        note.y,
        body_bottom
    );
    assert!(note.y <= page.height - 7200, "inside the bottom margin");
}

#[test]
fn the_signature_field_lands_in_a_box_and_its_anchor_is_written_back() {
    let doc = kitchen_sink();
    let layout = support::laid_out(&doc);
    let [sig] = layout.signatures.as_slice() else {
        panic!("one signature field, got {:?}", layout.signatures);
    };
    assert_eq!((sig.rect.width, sig.rect.height), (21_600, 3_600));
    assert_eq!(sig.page, 0);
    let mut anchored = doc.clone();
    write_anchors(&mut anchored, &layout);
    let anchor = anchored
        .inlines()
        .find_map(|i| match &i.content {
            InlineContent::SignatureField(f) => f.anchor,
            _ => None,
        })
        .expect("anchor written");
    assert_eq!((anchor.x.0, anchor.y.0), (sig.rect.x, sig.rect.y));
    assert_ne!(
        anchored, doc,
        "the fixture's placeholder anchor was replaced"
    );
}

#[test]
fn headings_links_images_and_labels_reach_the_ir() {
    let layout = support::laid_out(&kitchen_sink());
    assert_eq!(layout.outline.len(), 1);
    assert_eq!(layout.outline[0].title, "Every kind");
    let links: Vec<_> = layout.pages.iter().flat_map(|p| &p.links).collect();
    assert!(links.iter().any(|l| l.href == "https://avarok.net"));
    let image = layout.pages[0].draws.iter().find_map(|d| match d {
        Draw::Image { rect, content_id } => Some((rect.width, rect.height, content_id.as_str())),
        _ => None,
    });
    assert_eq!(image, Some((14_400, 7_200, "img-1")));
    assert!(layout.pages[1].text().contains("3.\nsecond level"));
}

#[test]
fn list_labels_follow_prosemirror_paragraph_order() {
    let doc = kitchen_sink();
    let labels = list_labels(&doc).expect("labels");
    let mut expected_len = 0;
    for section in &doc.sections {
        let p = &section.properties;
        for (_, hf) in p.headers.present().into_iter().chain(p.footers.present()) {
            expected_len += hf.blocks.iter().flat_map(Block::paragraphs).count();
        }
        expected_len += section.blocks.iter().flat_map(Block::paragraphs).count();
    }
    expected_len += doc
        .footnotes
        .iter()
        .flat_map(|f| &f.blocks)
        .flat_map(Block::paragraphs)
        .count();
    assert_eq!(labels.len(), expected_len);
    let numbered: Vec<_> = labels.iter().flatten().collect();
    assert_eq!(numbered, ["3."], "level 2 restarts at the override's 3");
    // Headers (3) and the default footer (1) come first in section 0.
    assert_eq!(labels.iter().position(Option::is_some), Some(4 + 3));
}

//! Breaking into pages and columns: forced breaks, keep rules, widow and orphan control,
//! columns, headers and footers with page numbers, and footnotes.

mod support;

use support::{para_with, text};

use cdoc_model::builtin::ids;
use cdoc_model::{Block, BreakKind, Field, FieldKind, HeaderFooter, Inline, InlineContent};

/// Paragraphs `filler-0` .. that fill most of an A4 page (about 34 single-line paragraphs).
fn filler(count: usize) -> Vec<Block> {
    (0..count)
        .map(|i| support::para(&format!("filler-{i}")))
        .collect()
}

fn page_of(layout: &cdoc_layout::Layout, needle: &str) -> usize {
    layout
        .pages
        .iter()
        .position(|p| p.runs().any(|r| r.text.contains(needle)))
        .unwrap_or_else(|| panic!("{needle:?} is on no page"))
}

#[test]
fn a_page_break_inside_a_paragraph_and_page_break_before_both_start_pages() {
    let mut blocks = vec![para_with(
        None,
        vec![
            text("before"),
            Inline::plain(InlineContent::Break {
                kind: BreakKind::Page,
            }),
            text("after"),
        ],
    )];
    let mut third = support::para("third");
    if let Block::Paragraph(p) = &mut third {
        p.properties.page_break_before = Some(true);
    }
    blocks.push(third);
    let layout = support::laid_out(&support::doc_with(blocks));
    assert_eq!(layout.pages.len(), 3);
    assert_eq!(page_of(&layout, "before"), 0);
    assert_eq!(page_of(&layout, "after"), 1);
    assert_eq!(page_of(&layout, "third"), 2);
}

#[test]
fn text_flows_onto_a_new_page_and_never_below_the_bottom_margin() {
    let layout = support::laid_out(&support::doc_with(filler(80)));
    assert!(layout.pages.len() >= 2);
    for page in &layout.pages {
        for run in page.runs() {
            assert!(
                run.y <= page.height - 7200,
                "{:?} at {} is in the margin",
                run.text,
                run.y
            );
            assert!(run.y >= 7200, "{:?} above the top margin", run.text);
        }
    }
    assert!(layout
        .warnings
        .iter()
        .all(|w| !matches!(w, cdoc_layout::Warning::Oversized { .. })));
}

#[test]
fn a_heading_at_the_foot_of_a_page_moves_down_with_its_paragraph() {
    let lines_per_page = support::laid_out(&support::doc_with(filler(80))).pages[0]
        .runs()
        .count();
    for before in lines_per_page.saturating_sub(4)..lines_per_page {
        let mut blocks = filler(before);
        blocks.push(para_with(
            Some(ids::heading(1).expect("1")),
            vec![text("Heading")],
        ));
        blocks.push(support::para("its paragraph"));
        let layout = support::laid_out(&support::doc_with(blocks));
        assert_eq!(
            page_of(&layout, "Heading"),
            page_of(&layout, "its paragraph"),
            "{before} fillers: kept with the next paragraph"
        );
    }
}

#[test]
fn widow_control_never_strands_one_line_of_a_paragraph() {
    let lines_per_page = support::laid_out(&support::doc_with(filler(80))).pages[0]
        .runs()
        .count();
    let long = "word ".repeat(60);
    for before in (lines_per_page.saturating_sub(6))..lines_per_page {
        let mut blocks = filler(before);
        blocks.push(support::para(&long));
        let layout = support::laid_out(&support::doc_with(blocks));
        let filler_y: Vec<i32> = layout.pages[0]
            .runs()
            .filter(|r| r.text.starts_with("filler"))
            .map(|r| r.y)
            .collect();
        let on_first = layout.pages[0]
            .runs()
            .filter(|r| r.text.starts_with("word") && !filler_y.contains(&r.y))
            .count();
        let on_second = layout.pages.get(1).map_or(0, |p| {
            p.runs().filter(|r| r.text.starts_with("word")).count()
        });
        assert!(
            on_first != 1 && on_second != 1,
            "{before} fillers: {on_first} + {on_second} lines"
        );
    }
}

#[test]
fn two_columns_fill_left_then_right_before_a_new_page() {
    let mut doc = support::doc_with(filler(50));
    doc.sections[0].properties.page.columns.count = 2;
    let layout = support::laid_out(&doc);
    assert_eq!(layout.pages.len(), 1, "50 lines fit in two columns");
    let xs: std::collections::BTreeSet<i32> = layout.pages[0].runs().map(|r| r.x).collect();
    assert_eq!(xs.len(), 2, "two column origins: {xs:?}");
    let right = *xs.iter().max().expect("x");
    assert!(right > 59530 / 2, "the second column is on the right half");
}

#[test]
fn footers_carry_page_numbers_and_the_page_count() {
    let mut doc = support::doc_with(filler(80));
    doc.sections[0].properties.footers.default = Some(HeaderFooter {
        blocks: vec![para_with(
            Some(ids::footer()),
            vec![
                text("Page "),
                Inline::plain(InlineContent::Field(Field {
                    field: FieldKind::PageNumber,
                    cached: "1".to_owned(),
                })),
                text(" of "),
                Inline::plain(InlineContent::Field(Field {
                    field: FieldKind::PageCount,
                    cached: "1".to_owned(),
                })),
            ],
        )],
    });
    doc.sections[0].properties.page.page_numbering.start = Some(5);
    let layout = support::laid_out(&doc);
    let n = layout.pages.len();
    for (i, page) in layout.pages.iter().enumerate() {
        let text = page.text();
        assert!(text.contains(&format!("Page {} of {n}", i + 5)), "{text}");
    }
}

#[test]
fn a_body_page_number_field_shows_the_page_it_lands_on() {
    let mut blocks = filler(80);
    blocks.push(para_with(
        None,
        vec![
            text("here: "),
            Inline::plain(InlineContent::Field(Field {
                field: FieldKind::PageNumber,
                cached: "9".to_owned(),
            })),
        ],
    ));
    let layout = support::laid_out(&support::doc_with(blocks));
    let last = layout.pages.last().expect("pages");
    assert!(
        last.runs()
            .any(|r| r.text == layout.pages.len().to_string()),
        "{}",
        last.text()
    );
}

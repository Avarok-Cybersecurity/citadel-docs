//! A line keeps only the caret stops no run draws; the stops of its text come from its runs,
//! on demand, so a layout for export computes none of them. The runs a page's lines own are
//! exactly its runs of document text (a repeated header row's copy is none), and a line's caret
//! spans its tallest text.

mod support;

use cdoc_layout::caret_stops;
use cdoc_model::{Inline, InlineContent};

fn gaps(stops: &[cdoc_layout::CaretStop]) -> Vec<u32> {
    stops.iter().map(|s| s.gap).collect()
}

#[test]
fn a_line_of_text_keeps_no_stops_of_its_own_and_its_runs_give_them() {
    let doc = support::doc_with(vec![support::para_with(
        None,
        vec![
            support::text("ab cd"),
            Inline {
                content: InlineContent::Tab,
                ..support::text("")
            },
            support::text("e"),
        ],
    )]);
    let laid = support::laid_out(&doc);
    let line = &laid.pages[0].lines[0];
    assert!(line.runs >= 1, "the text is runs");
    assert_eq!(gaps(&line.stops), [5, 6], "only the tab's stops are kept");
    assert_eq!(
        gaps(&caret_stops(&laid.pages[0]).expect("runs")[0]),
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
}

#[test]
fn an_empty_footnote_has_its_one_stop_after_its_number() {
    let id = cdoc_model::FootnoteId(1);
    let mut doc = support::doc_with(vec![support::para_with(
        None,
        vec![
            support::text("a"),
            Inline {
                content: InlineContent::FootnoteRef { id },
                ..support::text("")
            },
        ],
    )]);
    doc.footnotes.push(cdoc_model::Footnote {
        id,
        blocks: vec![support::para("")],
    });
    let laid = support::laid_out(&doc);
    let page = &laid.pages[0];
    let stops = caret_stops(page).expect("runs");
    let (k, note) = page
        .lines
        .iter()
        .enumerate()
        .find(|(_, l)| matches!(l.from.region, cdoc_layout::Region::Footnote(_)))
        .expect("the note is set");
    assert_eq!(note.runs, 0, "the number is no run of the note's text");
    assert_eq!(gaps(&stops[k]), [0]);
    let body_left = stops[0][0].x;
    assert!(
        stops[k][0].x > body_left,
        "the stop stands after the number"
    );
}

#[test]
fn a_line_is_as_tall_as_its_tallest_text_whatever_comes_after_it() {
    let sized = |s: &str, points| {
        let mut p = cdoc_model::RunProperties::none();
        p.size = Some(cdoc_model::FontSize::from_points(points));
        Inline::text(s, p)
    };
    let mixed = support::doc_with(vec![support::para_with(
        None,
        vec![sized("small ", 9), sized("TALL", 30), sized(" small", 9)],
    )]);
    let tall = support::doc_with(vec![support::para_with(None, vec![sized("TALL", 30)])]);
    let caret = |doc| {
        let laid = support::laid_out(doc);
        let line = &laid.pages[0].lines[0];
        line.bottom - line.top
    };
    assert_eq!(
        caret(&mixed),
        caret(&tall),
        "the caret spans the 30 pt text"
    );
}

#[test]
fn a_header_row_repeated_on_a_continuation_page_carries_no_lines() {
    use cdoc_model::{
        CellProperties, Length, Table, TableCell, TableProperties, TableRow, TableWidth,
    };
    let row = |text: &str, header: bool| TableRow {
        height: None,
        cant_split: false,
        cells: vec![TableCell {
            properties: CellProperties::single(),
            header,
            blocks: vec![support::para(text)],
        }],
    };
    let mut rows = vec![row("Heading", true)];
    rows.extend((0..80).map(|k| row(&format!("row {k}"), false)));
    let table = cdoc_model::Block::Table(Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length::from_twips(4000)],
        rows,
    });
    let laid = support::laid_out(&support::doc_with(vec![table]));
    assert!(laid.pages.len() > 1, "the table continues");
    assert!(
        laid.pages[1].text().contains("Heading"),
        "the header row repeats"
    );
    let heading = |page: &cdoc_layout::Page| {
        page.lines
            .iter()
            .filter(|l| l.from.path == [0, 0, 0, 0])
            .count()
    };
    assert_eq!(heading(&laid.pages[0]), 1);
    assert!(laid.pages[1..].iter().all(|p| heading(p) == 0));
    for (page, stops) in laid
        .pages
        .iter()
        .map(|p| (p, caret_stops(p).expect("runs")))
    {
        for (line, stops) in page.lines.iter().zip(stops) {
            let row = line.from.path[1] as usize;
            let len = if row == 0 {
                7
            } else {
                format!("row {}", row - 1).len()
            };
            assert_eq!(
                gaps(&stops),
                (0..=u32::try_from(len).expect("short")).collect::<Vec<_>>(),
                "row {row}"
            );
        }
    }
}

/// A footnote whose paragraph is a list item opening with a tab: its number, label and tab
/// each take a line, and the lines past the number's kept only a stop before the number,
/// which taking the number out dropped, leaving lines no caret could stand on (and display
/// lists the decoder refuses). Every line keeps a stop, at any width.
#[test]
fn every_line_of_a_footnote_list_item_has_a_caret_stop() {
    let id = cdoc_model::FootnoteId(1);
    for (width, list) in [(600, false), (600, true), (30_000, true)] {
        let mut doc = support::doc_with(vec![support::para_with(
            None,
            vec![
                support::text("a"),
                Inline {
                    content: InlineContent::FootnoteRef { id },
                    ..support::text("")
                },
            ],
        )]);
        let mut properties = cdoc_model::ParagraphProperties::none();
        if list {
            let kind = cdoc_model::builtin::ListKind::Decimal;
            doc.numbering
                .definitions
                .push(cdoc_model::AbstractNumbering {
                    id: cdoc_model::AbstractNumId(1),
                    levels: kind.levels(),
                });
            doc.numbering.instances.push(cdoc_model::NumberingInstance {
                id: cdoc_model::NumId(1),
                abstract_id: cdoc_model::AbstractNumId(1),
                overrides: Vec::new(),
            });
            properties.numbering = Some(cdoc_model::NumberingRef {
                num_id: cdoc_model::NumId(1),
                level: 8,
            });
        }
        doc.footnotes.push(cdoc_model::Footnote {
            id,
            blocks: vec![cdoc_model::Block::Paragraph(cdoc_model::Paragraph {
                properties,
                inlines: vec![Inline {
                    content: InlineContent::Tab,
                    ..support::text("")
                }],
            })],
        });
        let page = &mut doc.sections[0].properties.page;
        let side = cdoc_model::Length((page.size.width.0 - width) / 2);
        (page.margins.left, page.margins.right) = (side, side);
        let laid = support::laid_out(&doc);
        for (p, page) in laid.pages.iter().enumerate() {
            for (k, line) in page.lines.iter().enumerate() {
                assert!(
                    line.runs > 0 || !line.stops.is_empty(),
                    "width {width} list {list}: page {p} line {k} has no caret position"
                );
            }
        }
    }
}

#[test]
fn spaces_ending_a_paragraph_hang_past_its_last_word_and_the_caret_stands_after_each() {
    let laid = support::laid_out(&support::doc_with(vec![support::para("ab  ")]));
    let stops = &caret_stops(&laid.pages[0]).expect("runs")[0];
    assert_eq!(
        gaps(stops),
        [0, 1, 2, 3, 4],
        "each space is a place the caret stands"
    );
    let xs: Vec<_> = stops.iter().map(|s| s.x).collect();
    assert!(
        xs.windows(2).all(|w| w[0] < w[1]),
        "each stop is past the one before: {xs:?}"
    );
    let bare = support::laid_out(&support::doc_with(vec![support::para("ab")]));
    assert_eq!(
        laid.pages[0].draws, bare.pages[0].draws,
        "nothing is drawn for the hung spaces"
    );
}

#[test]
fn spaces_before_a_line_break_hang_too() {
    let doc = support::doc_with(vec![support::para_with(
        None,
        vec![
            support::text("ab "),
            Inline::plain(InlineContent::Break {
                kind: cdoc_model::BreakKind::Line,
            }),
            support::text("c"),
        ],
    )]);
    let laid = support::laid_out(&doc);
    let first = &caret_stops(&laid.pages[0]).expect("runs")[0];
    let at = |gap: u32| first.iter().find(|s| s.gap == gap).map(|s| s.x);
    assert!(at(3) > at(2), "the caret stands past the space: {first:?}");
}

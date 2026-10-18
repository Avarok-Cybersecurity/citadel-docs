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

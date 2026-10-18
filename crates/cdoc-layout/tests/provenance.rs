//! Every line of document text says where it came from and where a caret stands along it:
//! provenance resolves to a paragraph of the document, gaps count the paragraph's units as the
//! document holds them (hidden ones included), and what is drawn but no unit has no stop.

mod support;

use cdoc_layout::{caret_stops, layout, CaretLine, CaretStop, Layout, Page, Region, RevisionView};
use cdoc_model::{
    Block, Document, Inline, InlineContent, Marks, Paragraph, Revision, RevisionId, RevisionKind,
};

/// The paragraph a provenance names, read from the model.
fn paragraph<'d>(doc: &'d Document, line: &CaretLine) -> Option<&'d Paragraph> {
    let blocks: &[Block] = match line.from.region {
        Region::Body { section } => &doc.sections.get(section as usize)?.blocks,
        Region::Header { section, kind } => {
            let set = &doc.sections.get(section as usize)?.properties.headers;
            &set.present()
                .into_iter()
                .find(|(k, _)| *k == kind)?
                .1
                .blocks
        }
        Region::Footer { section, kind } => {
            let set = &doc.sections.get(section as usize)?.properties.footers;
            &set.present()
                .into_iter()
                .find(|(k, _)| *k == kind)?
                .1
                .blocks
        }
        Region::Footnote(id) => &doc.footnotes.iter().find(|n| n.id == id)?.blocks,
    };
    let (&first, rest) = line.from.path.split_first()?;
    let mut block = blocks.get(first as usize)?;
    for step in rest.chunks(3) {
        let Block::Table(table) = block else {
            return None;
        };
        let row = table.rows.get(step[0] as usize)?;
        block = row
            .cells
            .get(step[1] as usize)?
            .blocks
            .get(step[2] as usize)?;
    }
    match block {
        Block::Paragraph(p) => Some(p),
        Block::Table(_) => None,
    }
}

fn units(p: &Paragraph) -> u32 {
    p.inlines
        .iter()
        .map(|i| match &i.content {
            InlineContent::Text(run) => u32::try_from(run.text.chars().count()).expect("short"),
            _ => 1,
        })
        .sum()
}

fn laid_with(doc: &Document, revisions: RevisionView) -> Layout {
    let mut options = support::options();
    options.revisions = revisions;
    layout(doc, &support::fonts(), &options).expect("lays out")
}

/// Every line of `page` with its stops, as an editor asks for them.
fn lines(page: &Page) -> Vec<(&CaretLine, Vec<CaretStop>)> {
    let stops = caret_stops(page).expect("the page's runs are its lines'");
    page.lines.iter().zip(stops).collect()
}

fn all_lines(laid: &Layout) -> Vec<(&CaretLine, Vec<CaretStop>)> {
    laid.pages.iter().flat_map(lines).collect()
}

fn gaps(stops: &[CaretStop]) -> Vec<u32> {
    stops.iter().map(|s| s.gap).collect()
}

#[test]
fn every_line_of_the_kitchen_sink_names_a_paragraph_and_gaps_within_it() {
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    for view in [RevisionView::Markup, RevisionView::Final] {
        let laid = laid_with(&doc, view);
        let lines = all_lines(&laid);
        for (line, stops) in &lines {
            let p = paragraph(&doc, line).unwrap_or_else(|| panic!("{:?} resolves", line.from));
            stops.first().expect("a line has a stop");
            assert!(line.top < line.bottom);
            let g = gaps(stops);
            assert!(g.windows(2).all(|w| w[0] < w[1]), "{g:?} increase");
            assert!(
                g.iter().all(|&k| k <= units(p)),
                "{g:?} within {}",
                units(p)
            );
        }
        let region = |f: fn(&Region) -> bool| lines.iter().any(|(l, _)| f(&l.from.region));
        assert!(region(|r| matches!(r, Region::Header { .. })));
        assert!(region(|r| matches!(r, Region::Footnote(_))));
        assert!(region(|r| matches!(r, Region::Body { section: 1 })));
        assert!(
            lines.iter().any(|(l, _)| l.from.path.len() == 7),
            "a paragraph of the nested table"
        );
    }
}

#[test]
fn a_line_break_ends_a_line_at_its_own_gap_and_an_empty_paragraph_has_one_stop() {
    let doc = support::doc_with(vec![support::para("ab\ncd"), support::para("")]);
    let laid = support::laid_out(&doc);
    let lines = lines(&laid.pages[0]);
    let body: Vec<(Vec<u32>, Vec<u32>)> = lines
        .iter()
        .map(|(l, s)| (l.from.path.clone(), gaps(s)))
        .collect();
    assert_eq!(
        body,
        [
            (vec![0], vec![0, 1, 2]),
            (vec![0], vec![3, 4, 5]),
            (vec![1], vec![0])
        ]
    );
    let left = lines[0].1[0].x;
    assert_eq!(lines[2].1[0].x, left, "the empty line starts at the margin");
}

#[test]
fn hidden_deleted_text_keeps_its_units_and_has_no_stops() {
    let deleted = Marks {
        revision: Some(Revision {
            id: RevisionId::new("r1"),
            kind: RevisionKind::Deletion,
            author: "A".to_owned(),
            date: "2026-10-18T00:00:00Z".to_owned(),
        }),
        ..Marks::none()
    };
    let gone = Inline {
        marks: deleted,
        ..support::text("XY")
    };
    let doc = support::doc_with(vec![support::para_with(
        None,
        vec![support::text("ab"), gone, support::text("cd")],
    )]);
    let laid = laid_with(&doc, RevisionView::Final);
    let stops = &lines(&laid.pages[0])[0].1;
    assert_eq!(gaps(stops), [0, 1, 2, 4, 5, 6]);
    assert_eq!(stops[2].x, stops[3].x, "nothing drawn between 2 and 4");
    let markup = laid_with(&doc, RevisionView::Markup);
    assert_eq!(gaps(&lines(&markup.pages[0])[0].1), [0, 1, 2, 3, 4, 5, 6]);
}

#[test]
fn a_footnotes_number_is_no_unit_of_its_paragraph() {
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let laid = support::laid_out(&doc);
    let lines = all_lines(&laid);
    let (_, note) = lines
        .iter()
        .find(|(l, _)| matches!(l.from.region, Region::Footnote(_)))
        .expect("the note is set");
    assert_eq!(gaps(note), (0..=6).collect::<Vec<u32>>(), "\"A note\"");
    let number_end = note[0].x;
    let column_left = lines
        .iter()
        .filter(|(l, _)| matches!(l.from.region, Region::Body { .. }))
        .map(|(_, s)| s[0].x)
        .min()
        .expect("body lines");
    assert!(number_end > column_left, "gap 0 stands after the number");
}

#[test]
fn a_footer_line_names_its_section_and_kind() {
    let mut doc = support::doc_with(vec![support::para("body")]);
    doc.sections[0].properties.footers.default = Some(cdoc_model::HeaderFooter {
        blocks: vec![support::para("foot"), support::para("note")],
    });
    let laid = support::laid_out(&doc);
    let foot: Vec<(Region, Vec<u32>, Vec<u32>)> = lines(&laid.pages[0])
        .into_iter()
        .filter(|(l, _)| !matches!(l.from.region, Region::Body { .. }))
        .map(|(l, s)| (l.from.region, l.from.path.clone(), gaps(&s)))
        .collect();
    let footer = Region::Footer {
        section: 0,
        kind: cdoc_model::HeaderFooterKind::Default,
    };
    assert_eq!(
        foot,
        [
            (footer, vec![0], vec![0, 1, 2, 3, 4]),
            (footer, vec![1], vec![0, 1, 2, 3, 4])
        ]
    );
}

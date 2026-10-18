//! Formatting at the caret: effective values through the style cascade, the direct patch, the
//! pending formatting of a collapsed caret, and the properties a selection holds mixed.

mod common;

use cdoc_crdt::RunKey;
use cdoc_edit::{Command, MarkKind, ParagraphKey};
use cdoc_model::{
    Alignment, FontSize, NumId, NumberingRef, ParagraphProperties, RunProperties, StyleId,
};
use common::{caret, editor, gap, paragraphs, select};

#[test]
fn a_caret_reports_the_effective_and_the_direct_formatting() {
    let mut e = editor(&paragraphs(&["alpha beta"]), 1);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, 5));
    select(&mut e, s, t);
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    let g = gap(&e, 0, 3);
    caret(&mut e, g);
    let fmt = e.caret_formatting().unwrap();
    assert_eq!(fmt.run_direct.bold, Some(true));
    assert_eq!(fmt.run.bold, Some(true));
    assert_eq!(fmt.run.size, Some(FontSize(22)), "the document default");
    assert_eq!(fmt.run_direct.size, None);
    assert_eq!(fmt.paragraph.style, Some(StyleId::new("Normal")));
    assert!(fmt.mixed_run.is_empty() && fmt.mixed_paragraph.is_empty());
}

#[test]
fn a_paragraph_style_shows_through_the_cascade() {
    let mut e = editor(&paragraphs(&["Heading"]), 1);
    e.apply(Command::SetParagraphStyle(StyleId::new("Heading1")))
        .unwrap();
    let fmt = e.caret_formatting().unwrap();
    assert_eq!(fmt.run.size, Some(FontSize(32)), "Heading 1 is 16 pt");
    assert_eq!(fmt.run_direct, RunProperties::none());
    assert_eq!(fmt.paragraph_direct.style, Some(StyleId::new("Heading1")));
}

#[test]
fn a_toggle_at_a_collapsed_caret_shows_before_anything_is_typed() {
    let mut e = editor(&paragraphs(&["plain"]), 1);
    let g = gap(&e, 0, 2);
    caret(&mut e, g);
    assert_eq!(e.caret_formatting().unwrap().run.italic, None);
    e.apply(Command::ToggleMark(MarkKind::Italic)).unwrap();
    assert_eq!(e.caret_formatting().unwrap().run.italic, Some(true));
}

#[test]
fn a_selection_reports_what_differs_across_it() {
    let mut e = editor(&paragraphs(&["one two", "three"]), 1);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, 3));
    select(&mut e, s, t);
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    e.apply(Command::SetParagraphProperty(
        (&ParagraphProperties {
            alignment: Some(Alignment::Center),
            ..ParagraphProperties::none()
        })
            .into(),
    ))
    .unwrap();
    let (s, t) = (gap(&e, 0, 1), gap(&e, 1, 2));
    select(&mut e, s, t);
    let fmt = e.caret_formatting().unwrap();
    assert_eq!(
        fmt.run.bold,
        Some(true),
        "the selection starts in bold text"
    );
    assert_eq!(fmt.mixed_run, vec![RunKey::Bold]);
    assert_eq!(fmt.mixed_paragraph, vec![ParagraphKey::Alignment]);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, 3));
    select(&mut e, s, t);
    let fmt = e.caret_formatting().unwrap();
    assert!(fmt.mixed_run.is_empty() && fmt.mixed_paragraph.is_empty());
}

#[test]
fn a_list_item_reports_its_list_and_level() {
    let mut d = paragraphs(&["item", "plain"]);
    d.numbering = cdoc_model::test_support::fixtures::kitchen_sink().numbering;
    let mut e = editor(&d, 1);
    e.apply(Command::SetNumbering {
        num_id: NumId(1),
        level: 1,
    })
    .unwrap();
    let want = Some(NumberingRef {
        num_id: NumId(1),
        level: 1,
    });
    assert_eq!(e.caret_formatting().unwrap().paragraph.numbering, want);
    let (s, t) = (gap(&e, 0, 1), gap(&e, 1, 1));
    select(&mut e, s, t);
    assert_eq!(
        e.caret_formatting().unwrap().mixed_paragraph,
        vec![ParagraphKey::Numbering]
    );
}

/// What the caret reports as the direct formatting is what typing there produces, at every
/// caret stop: body, tables, headers, footers and footnotes.
#[test]
fn the_reported_formatting_is_what_typing_gets_everywhere() {
    let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let stops: Vec<_> = (0..e.page_count())
        .flat_map(|page| e.stops(page))
        .map(|(p, _)| p)
        .collect();
    assert!(stops.len() > 20);
    for p in stops {
        e.apply(Command::SetSelection(cdoc_edit::Selection::caret(p)))
            .unwrap();
        let want = e.caret_formatting().unwrap().run_direct;
        let gap = p.index(e.doc()).unwrap();
        e.apply(Command::InsertText("x".into())).unwrap();
        let got = cdoc_crdt::slice(e.doc(), gap..gap + 1)
            .unwrap()
            .into_iter()
            .find_map(|piece| match piece {
                cdoc_crdt::Piece::Inline(i) => match i.content {
                    cdoc_model::InlineContent::Text(run) => Some(run.properties),
                    _ => None,
                },
                cdoc_crdt::Piece::Block(_) => None,
            })
            .expect("typed text");
        assert_eq!(got, want, "at {p:?}");
        e.undo().unwrap();
    }
}

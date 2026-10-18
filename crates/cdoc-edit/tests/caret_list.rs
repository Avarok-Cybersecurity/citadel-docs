//! The list at the caret, and the result record's story and list sections.

mod common;

use cdoc_edit::wire::{decode_result, encode_result};
use cdoc_edit::{CaretList, CaretListKind, Command, Story};
use cdoc_model::builtin::ListKind;
use cdoc_model::FootnoteId;
use cdoc_model::HeaderFooterKind;
use common::{caret, editor, gap, paragraphs, select};

fn list(e: &cdoc_edit::Editor<'_>) -> (Option<CaretList>, bool) {
    let f = e.caret_formatting().expect("formatting");
    (f.list, f.mixed_list)
}

#[test]
#[allow(
    clippy::many_single_char_names,
    reason = "gaps a to d, in document order"
)]
fn the_caret_reports_its_lists_kind_and_level() {
    let mut e = editor(&paragraphs(&["one", "two", "three", "plain"]), 1);
    assert_eq!(list(&e), (None, false));
    let (a, b) = (gap(&e, 0, 1), gap(&e, 1, 1));
    select(&mut e, a, b);
    e.apply(Command::SetNumberingKind {
        kind: ListKind::UpperRoman,
        level: 2,
    })
    .expect("lists");
    let roman = CaretList {
        kind: CaretListKind::Builtin(ListKind::UpperRoman),
        level: 2,
    };
    assert_eq!(
        list(&e),
        (Some(roman), false),
        "same kind and level throughout"
    );
    let c = gap(&e, 2, 1);
    caret(&mut e, c);
    e.apply(Command::SetNumberingKind {
        kind: ListKind::Bullet,
        level: 2,
    })
    .expect("lists");
    let (a, c) = (gap(&e, 0, 1), gap(&e, 2, 1));
    select(&mut e, a, c);
    assert_eq!(list(&e), (Some(roman), true), "a bullet item too");
    let (b, d) = (gap(&e, 1, 0), gap(&e, 3, 2));
    select(&mut e, b, d);
    assert_eq!(list(&e), (Some(roman), true), "and a plain paragraph");
    caret(&mut e, a);
    e.apply(Command::Indent).expect("indents");
    assert_eq!(list(&e).0.map(|l| l.level), Some(3));
}

#[test]
fn a_definition_no_template_matches_is_custom() {
    let mut e = editor(&paragraphs(&["one"]), 1);
    e.apply(Command::SetNumberingKind {
        kind: ListKind::Decimal,
        level: 0,
    })
    .expect("lists");
    let mut doc = e.model().clone();
    doc.numbering.definitions[0].levels[0].text = "(%1)".to_owned();
    let e = editor(&doc, 2);
    assert_eq!(
        list(&e).0,
        Some(CaretList {
            kind: CaretListKind::Custom,
            level: 0
        })
    );
}

#[test]
fn every_list_and_story_round_trips_in_the_result_record() {
    let e = editor(&paragraphs(&["one"]), 1);
    let base = e
        .outcome(Vec::new(), Vec::new(), Vec::new())
        .expect("an outcome");
    assert_eq!(base.story, Story::Body);
    let kinds = ListKind::ALL
        .map(CaretListKind::Builtin)
        .into_iter()
        .chain([CaretListKind::Custom]);
    let lists = kinds.flat_map(|kind| [0, 8].map(|level| Some(CaretList { kind, level })));
    let stories = [
        Story::Body,
        Story::Header {
            section: 0,
            kind: HeaderFooterKind::Default,
        },
        Story::Footer {
            section: 400,
            kind: HeaderFooterKind::Even,
        },
        Story::Header {
            section: 2,
            kind: HeaderFooterKind::First,
        },
        Story::Footnote(FootnoteId(90_000)),
    ];
    for (k, (list, story)) in lists.zip(stories.iter().cycle()).enumerate() {
        let mut o = base.clone();
        o.formatting.list = list;
        o.formatting.mixed_list = k % 2 == 1;
        o.story = *story;
        let bytes = encode_result(&o);
        assert_eq!(decode_result(&bytes).expect("decodes"), o);
        for cut in 0..bytes.len() {
            assert!(decode_result(&bytes[..cut]).is_err());
        }
    }
}

#[test]
fn bad_list_and_story_tags_are_refused() {
    let e = editor(&paragraphs(&["one"]), 1);
    let mut o = e
        .outcome(Vec::new(), Vec::new(), Vec::new())
        .expect("an outcome");
    o.story = Story::Footnote(FootnoteId(5));
    let bytes = encode_result(&o);
    let story = bytes.len() - 2;
    assert_eq!(bytes[story..], [3, 5]);
    for tag in [4, 0xff] {
        let mut bad = bytes.clone();
        bad[story] = tag;
        assert!(decode_result(&bad).is_err(), "story tag {tag}");
        let mut bad = bytes[..story].to_vec();
        bad.push(tag);
        assert!(decode_result(&bad).is_err(), "story tag {tag} alone");
    }
    o.story = Story::Header {
        section: 1,
        kind: HeaderFooterKind::Even,
    };
    let bytes = encode_result(&o);
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() = 3;
    assert!(decode_result(&bad).is_err(), "header kind 3");
    o.story = Story::Body;
    o.formatting.list = Some(CaretList {
        kind: CaretListKind::Custom,
        level: 8,
    });
    let bytes = encode_result(&o);
    let at = bytes
        .windows(4)
        .rposition(|w| w == [1, 6, 8, 0])
        .expect("the list section");
    for (offset, value) in [(0, 2), (1, 7), (2, 9), (3, 2)] {
        let mut bad = bytes.clone();
        bad[at + offset] = value;
        assert!(decode_result(&bad).is_err(), "byte {offset} = {value}");
    }
}

//! List labels from the model's numbering: the one implementation the UI uses through
//! `cdoc-wasm`.

mod support;

use cdoc_layout::labels::format_number;
use cdoc_layout::list_labels;
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Block, Level, NumId, NumberFormat, Numbering,
    NumberingInstance, NumberingRef, Paragraph, ParagraphProperties, RunProperties,
};

fn level(text: &str, format: NumberFormat, start: u32) -> Level {
    Level {
        start,
        format,
        text: text.to_owned(),
        alignment: Alignment::Left,
        indent: None,
        run: None,
    }
}

fn item(level: u8) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            numbering: Some(NumberingRef {
                num_id: NumId(1),
                level,
            }),
            ..ParagraphProperties::none()
        },
        inlines: vec![cdoc_model::Inline::text("x", RunProperties::none())],
    })
}

fn numbered(levels: Vec<Level>, items: &[u8]) -> Vec<Option<String>> {
    let mut blocks: Vec<Block> = items.iter().map(|l| item(*l)).collect();
    blocks.insert(1, support::para("not in the list"));
    let mut doc = support::doc_with(blocks);
    doc.numbering = Numbering {
        definitions: vec![AbstractNumbering {
            id: AbstractNumId(1),
            levels,
        }],
        instances: vec![NumberingInstance {
            id: NumId(1),
            abstract_id: AbstractNumId(1),
            overrides: vec![],
        }],
    };
    list_labels(&doc).expect("labels")
}

#[test]
fn multilevel_labels_nest_and_deeper_levels_restart() {
    let labels = numbered(
        vec![
            level("%1.", NumberFormat::Decimal, 1),
            level("%1.%2", NumberFormat::Decimal, 1),
        ],
        &[0, 1, 1, 0, 1],
    );
    let got: Vec<Option<&str>> = labels.iter().map(Option::as_deref).collect();
    assert_eq!(
        got,
        [
            Some("1."),
            None,
            Some("1.1"),
            Some("1.2"),
            Some("2."),
            Some("2.1")
        ]
    );
}

#[test]
fn letters_romans_and_bullets_render_in_their_formats() {
    let labels = numbered(
        vec![
            level("%1)", NumberFormat::LowerLetter, 1),
            level("%2.", NumberFormat::UpperRoman, 4),
            level("\u{2022}", NumberFormat::Bullet, 1),
        ],
        &[0, 1, 1, 2, 0],
    );
    let got: Vec<Option<&str>> = labels.iter().map(Option::as_deref).collect();
    assert_eq!(
        got,
        [
            Some("a)"),
            None,
            Some("IV."),
            Some("V."),
            Some("\u{2022}"),
            Some("b)")
        ]
    );
}

#[test]
fn number_formats_follow_word() {
    assert_eq!(format_number(27, NumberFormat::UpperLetter), "AA");
    assert_eq!(format_number(53, NumberFormat::LowerLetter), "aaa");
    assert_eq!(format_number(1994, NumberFormat::UpperRoman), "MCMXCIV");
    assert_eq!(format_number(9, NumberFormat::LowerRoman), "ix");
    assert_eq!(format_number(7, NumberFormat::None), "");
}

#[test]
fn the_label_is_drawn_in_the_hanging_indent_before_the_text() {
    let mut doc = support::doc_with(vec![item(0)]);
    let mut first = level("%1.", NumberFormat::Decimal, 1);
    first.indent = Some(cdoc_model::Indent {
        left: Some(cdoc_model::Length(3600)),
        right: None,
        first_line: Some(cdoc_model::FirstLine::Hanging {
            by: cdoc_model::Length(1800),
        }),
    });
    doc.numbering = Numbering {
        definitions: vec![AbstractNumbering {
            id: AbstractNumId(1),
            levels: vec![first],
        }],
        instances: vec![NumberingInstance {
            id: NumId(1),
            abstract_id: AbstractNumId(1),
            overrides: vec![],
        }],
    };
    let layout = support::laid_out(&doc);
    let runs: Vec<_> = layout.pages[0].runs().collect();
    assert_eq!((runs[0].text.as_str(), runs[0].x), ("1.", 7200 + 1800));
    assert_eq!((runs[1].text.as_str(), runs[1].x), ("x", 7200 + 3600));
}

#[test]
fn a_label_carries_the_counter_of_its_own_level() {
    use cdoc_layout::labels::Counters;
    let numbering = Numbering {
        definitions: vec![AbstractNumbering {
            id: AbstractNumId(1),
            levels: vec![
                level("%1.", NumberFormat::UpperRoman, 1),
                level("%1.%2", NumberFormat::LowerLetter, 3),
            ],
        }],
        instances: vec![NumberingInstance {
            id: NumId(1),
            abstract_id: AbstractNumId(1),
            overrides: vec![],
        }],
    };
    let mut counters = Counters::new(&numbering);
    let reference = |level| NumberingRef {
        num_id: NumId(1),
        level,
    };
    let numbers: Vec<u32> = [0, 0, 1, 1, 0]
        .into_iter()
        .map(|l| counters.next(reference(l)).expect("label").number)
        .collect();
    // the second level starts at 3 and restarts under every new first-level item
    assert_eq!(numbers, [1, 2, 3, 4, 3]);
}

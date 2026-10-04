//! The documented mapping, pinned: the kitchen-sink document as Markdown, and what the writer
//! reports it could not carry.

mod common;

use cdoc_md::document_to_md;
use cdoc_model::test_support::fixtures::kitchen_sink;

const EXPECTED: &str = "# Kitchen sink

# Every **kind**

Body [***a link***](<https://avarok.net> \"Avarok\")\t![a picture](<cid:img-1>)\\
[^1]4 October 2026no

3. second level

| Merged header |  |
| --- | --- |
| a | nested |

> Quoted

Second section

[^1]: A note
";

#[test]
fn kitchen_sink_as_markdown() {
    let (md, _) = document_to_md(&kitchen_sink());
    assert_eq!(md, EXPECTED);
}

#[test]
fn kitchen_sink_report_names_each_kind_of_loss_once_per_occurrence() {
    let (_, report) = document_to_md(&kitchen_sink());
    let expected: &[(&str, u32)] = &[
        ("document title (metadata)", 1),
        ("section page setup", 2),
        ("header", 6),
        ("footer", 2),
        ("Title style (written as a level 1 heading)", 1),
        ("comment anchor", 1),
        ("tracked insertion (accepted)", 1),
        ("tracked deletion (text removed)", 1),
        ("underline", 1),
        ("text colour", 1),
        ("highlight", 1),
        ("font size", 1),
        ("font family", 1),
        ("superscript or subscript", 1),
        ("small or all capitals", 1),
        ("field (its cached text is written)", 1),
        ("form field (written as its value)", 1),
        ("signature field (dropped)", 1),
        ("page or column break", 1),
        ("merged cell (flattened)", 1),
        ("comment", 2),
    ];
    for (what, count) in expected {
        assert_eq!(report.count_of(what), *count, "{what}: {report:?}");
    }
}

#[test]
fn emphasis_on_edge_punctuation_moves_outside_and_is_reported() {
    use cdoc_model::{Block, Document, Inline, RunProperties};
    let bold = RunProperties {
        bold: Some(true),
        ..RunProperties::none()
    };
    let mut doc = Document::blank();
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines = vec![
        Inline::text("a", RunProperties::none()),
        Inline::text("\"quoted\"", bold),
        Inline::text("b", RunProperties::none()),
    ];
    let (md, report) = document_to_md(&doc);
    assert_eq!(md, "a\"**quoted**\"b\n");
    assert_eq!(
        report.count_of("emphasis on edge punctuation (moved outside the markers)"),
        1
    );
}

#[test]
fn lists_as_markdown_use_counted_numbers_and_nest_four_spaces() {
    use cdoc_model::{
        AbstractNumId, AbstractNumbering, Alignment, Block, Document, Inline, Level, NumId,
        NumberFormat, NumberingInstance, NumberingRef, Paragraph, ParagraphProperties,
        RunProperties, StyleId,
    };
    let level = |format, text: &str| Level {
        start: 1,
        format,
        text: text.to_owned(),
        alignment: Alignment::Left,
        indent: None,
        run: None,
    };
    let mut doc = Document::blank();
    doc.numbering.definitions = vec![
        AbstractNumbering {
            id: AbstractNumId(1),
            levels: vec![
                level(NumberFormat::LowerLetter, "%1)"),
                level(NumberFormat::Decimal, "%2."),
            ],
        },
        AbstractNumbering {
            id: AbstractNumId(2),
            levels: vec![level(NumberFormat::Bullet, "\u{2022}")],
        },
    ];
    doc.numbering.instances = vec![
        NumberingInstance {
            id: NumId(1),
            abstract_id: AbstractNumId(1),
            overrides: vec![],
        },
        NumberingInstance {
            id: NumId(2),
            abstract_id: AbstractNumId(2),
            overrides: vec![],
        },
    ];
    let item = |num, lvl, text: &str| {
        Block::Paragraph(Paragraph {
            properties: ParagraphProperties {
                style: Some(StyleId::new("ListParagraph")),
                numbering: Some(NumberingRef {
                    num_id: NumId(num),
                    level: lvl,
                }),
                ..ParagraphProperties::none()
            },
            inlines: vec![Inline::text(text, RunProperties::none())],
        })
    };
    doc.sections[0].blocks = vec![
        item(1, 0, "first"),
        item(1, 1, "inner"),
        item(1, 1, "inner two"),
        item(1, 0, "second"),
        item(2, 0, "bullet"),
    ];
    let (md, report) = document_to_md(&doc);
    // lettered numbering has no Markdown spelling, so the count is written in digits
    assert_eq!(
        md,
        "1. first\n    1. inner\n    2. inner two\n2. second\n- bullet\n"
    );
    assert_eq!(report.count_of("section page setup"), 1);
}

//! Reading plain text, and what writing it back does.

use cdoc_model::{Block, Document, InlineContent};
use cdoc_txt::{document_to_txt, txt_to_document, SingleNewline, TxtReadOptions};

/// Test options (PCND): tests state the meaning of a newline they rely on.
const BREAK: TxtReadOptions = TxtReadOptions {
    single_newline: SingleNewline::Break,
};
const SPACE: TxtReadOptions = TxtReadOptions {
    single_newline: SingleNewline::Space,
};

fn texts(doc: &Document) -> Vec<String> {
    doc.paragraphs()
        .map(|p| {
            p.inlines
                .iter()
                .map(|i| match &i.content {
                    InlineContent::Text(t) => t.text.clone(),
                    InlineContent::Tab => "<tab>".to_owned(),
                    InlineContent::Break { kind } => format!("<{kind:?}>"),
                    other => format!("{other:?}"),
                })
                .collect()
        })
        .collect()
}

#[test]
fn blank_lines_separate_paragraphs_and_single_newlines_follow_the_option() {
    let input = "one\ntwo\n\n\n  \nthree\r\nfour\rfive\n";
    assert_eq!(
        texts(&txt_to_document(input, &BREAK).0),
        ["one<Line>two", "three<Line>four<Line>five"]
    );
    assert_eq!(
        texts(&txt_to_document(input, &SPACE).0),
        ["one two", "three four five"]
    );
}

#[test]
fn tabs_and_form_feeds_are_kept_as_what_they_are() {
    let (doc, report) = txt_to_document("a\tb\u{c}c", &BREAK);
    assert_eq!(texts(&doc), ["a<tab>b<Page>c"]);
    assert!(report.is_lossless());
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn empty_and_blank_input_is_a_valid_blank_document() {
    for input in ["", "\n\n", "  \n \t \n"] {
        let (doc, report) = txt_to_document(input, &BREAK);
        assert_eq!(doc.validate(), Ok(()));
        assert!(report.is_lossless());
        assert_eq!(doc.word_count(), 0);
    }
}

#[test]
fn leading_and_trailing_spaces_on_a_line_survive() {
    let (doc, _) = txt_to_document("  indented  \n", &BREAK);
    assert_eq!(document_to_txt(&doc).0, "  indented  \n");
}

#[test]
fn lists_tables_and_footnotes_render_sensibly() {
    use cdoc_model::{
        AbstractNumId, AbstractNumbering, Alignment, CellProperties, Inline, Length, Level, NumId,
        NumberFormat, NumberingInstance, NumberingRef, Paragraph, ParagraphProperties,
        RunProperties, StyleId, Table, TableCell, TableProperties, TableRow, TableWidth,
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
    doc.numbering.definitions.push(AbstractNumbering {
        id: AbstractNumId(1),
        levels: vec![
            level(NumberFormat::Decimal, "%1."),
            level(NumberFormat::LowerLetter, "%2)"),
        ],
    });
    doc.numbering.definitions.push(AbstractNumbering {
        id: AbstractNumId(2),
        levels: vec![level(NumberFormat::Bullet, "\u{2022}")],
    });
    for (n, a) in [(1, 1), (2, 2)] {
        doc.numbering.instances.push(NumberingInstance {
            id: NumId(n),
            abstract_id: AbstractNumId(a),
            overrides: vec![],
        });
    }
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
    let cell = |text: &str, header, alignment| TableCell {
        properties: CellProperties::single(),
        header,
        blocks: vec![Block::Paragraph(Paragraph {
            properties: ParagraphProperties {
                alignment,
                ..ParagraphProperties::none()
            },
            inlines: vec![Inline::text(text, RunProperties::none())],
        })],
    };
    let row = |cells| TableRow {
        height: None,
        cant_split: false,
        cells,
    };
    let table = Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length(100); 2],
        rows: vec![
            row(vec![
                cell("Region", true, None),
                cell("Revenue", true, Some(Alignment::Right)),
            ]),
            row(vec![
                cell("North", false, None),
                cell("10", false, Some(Alignment::Right)),
            ]),
        ],
    };
    doc.sections[0].blocks = vec![
        item(1, 0, "first"),
        item(1, 1, "nested"),
        item(1, 0, "second"),
        item(2, 0, "bullet"),
        Block::Table(table),
    ];
    let (text, report) = document_to_txt(&doc);
    assert_eq!(
        text,
        "1. first\n  a) nested\n2. second\n- bullet\n\nRegion | Revenue\n-------+--------\nNorth  |      10\n"
    );
    assert!(report.is_lossless(), "{report:?}");
}

#[test]
fn real_world_text_reads_and_rewrites_stably() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        dir.join("tests/fixtures/pandoc.txt"),
        dir.join("../../ARCHITECTURE.md"),
        dir.join("../../LICENSE-MIT"),
    ];
    for path in files {
        let input = std::fs::read_to_string(&path).expect("readable fixture");
        let (doc, _) = txt_to_document(&input, &BREAK);
        assert_eq!(doc.validate(), Ok(()), "{path:?}");
        let words: usize = input.split_whitespace().count();
        assert_eq!(
            doc.word_count(),
            words,
            "{path:?}: no word is lost or invented"
        );
        let (written, _) = document_to_txt(&doc);
        let (again, _) = txt_to_document(&written, &BREAK);
        assert_eq!(
            document_to_txt(&again).0,
            written,
            "{path:?}: stable after one pass"
        );
    }
}

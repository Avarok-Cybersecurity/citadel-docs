//! Documents in the domain Markdown can hold exactly: headings, quotes, nested lists, tables
//! and paragraphs of runs. Emphasised runs begin and end on a letter or digit (Markdown cannot
//! open emphasis at punctuation next to a word; the writer moves such edges outside, which a
//! separate test covers); plain runs may hold any character.

use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Block, CellProperties, Document, FirstLine,
    Indent, Inline, InlineContent, Length, Level, Link, Marks, NumId, NumberFormat, Numbering,
    NumberingInstance, NumberingRef, Paragraph, ParagraphProperties, RunProperties, StyleId, Table,
    TableCell, TableProperties, TableRow, TableWidth, TextRun,
};
use proptest::prelude::*;

fn plain_text() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-zA-Z0-9 .,;:!?*_\\[\\]<>&#|\\\\`~()+=\\-]{1,10}")
        .expect("regex")
        .boxed()
}

fn word_edged() -> BoxedStrategy<String> {
    let mid =
        proptest::string::string_regex("[a-z0-9 .,*_<>&#|\\\\`~()\\[\\]]{0,6}").expect("regex");
    ("[a-z0-9]{1}", mid, "[a-z0-9]{1}")
        .prop_map(|(a, m, b)| format!("{a}{m}{b}"))
        .boxed()
}

fn run(code: bool) -> BoxedStrategy<Inline> {
    let flags = (any::<bool>(), any::<bool>(), any::<bool>());
    let link = prop::option::of((
        "[a-z]{1,6}".prop_map(|s| format!("https://example.test/{s}")),
        prop::option::of("[a-z ]{1,6}"),
    ));
    (flags, any::<bool>(), link, plain_text(), word_edged())
        .prop_map(
            move |((bold, italic, strike), plain, link, plain_text, edged)| {
                let emphasised = bold || italic || strike;
                let is_code = code && !emphasised && !plain;
                let text = if emphasised || is_code {
                    edged
                } else {
                    plain_text
                };
                let properties = RunProperties {
                    bold: bold.then_some(true),
                    italic: italic.then_some(true),
                    strike: strike.then_some(true),
                    font: is_code.then(|| cdoc_md::CODE_FONT.to_owned()),
                    ..RunProperties::none()
                };
                Inline {
                    content: InlineContent::Text(TextRun { text, properties }),
                    marks: Marks {
                        link: link.map(|(href, title)| Link { href, title }),
                        ..Marks::none()
                    },
                }
            },
        )
        .boxed()
}

fn inlines() -> BoxedStrategy<Vec<Inline>> {
    prop::collection::vec(run(true), 1..=4).boxed()
}

fn paragraph(style: Option<&str>, inlines: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style: style.map(StyleId::new),
            ..ParagraphProperties::none()
        },
        inlines,
    })
}

fn level(n: u8, format: NumberFormat) -> Level {
    Level {
        start: 1,
        format,
        text: if format == NumberFormat::Bullet {
            "\u{2022}".to_owned()
        } else {
            format!("%{}.", n + 1)
        },
        alignment: Alignment::Left,
        indent: Some(Indent {
            left: Some(Length::from_twips(720)),
            right: None,
            first_line: Some(FirstLine::Hanging {
                by: Length::from_twips(360),
            }),
        }),
        run: None,
    }
}

fn numbering() -> Numbering {
    let def = |id, format| AbstractNumbering {
        id: AbstractNumId(id),
        levels: (0..9).map(|n| level(n, format)).collect(),
    };
    let inst = |id, abs| NumberingInstance {
        id: NumId(id),
        abstract_id: AbstractNumId(abs),
        overrides: vec![],
    };
    Numbering {
        definitions: vec![def(1, NumberFormat::Bullet), def(2, NumberFormat::Decimal)],
        instances: vec![inst(1, 1), inst(2, 2)],
    }
}

/// A list: a run of items whose levels each exceed the previous by at most one.
fn list() -> BoxedStrategy<Vec<Block>> {
    prop::collection::vec((0u8..3, inlines()), 1..=4)
        .prop_flat_map(|items| (any::<bool>(), Just(items)))
        .prop_map(|(ordered, items)| {
            let mut above: Option<u8> = None;
            items
                .into_iter()
                .map(|(wanted, inlines)| {
                    let level = above.map_or(0, |a| wanted.min(a + 1));
                    above = Some(level);
                    Block::Paragraph(Paragraph {
                        properties: ParagraphProperties {
                            style: Some(StyleId::new("ListParagraph")),
                            numbering: Some(NumberingRef {
                                num_id: NumId(if ordered { 2 } else { 1 }),
                                level,
                            }),
                            ..ParagraphProperties::none()
                        },
                        inlines,
                    })
                })
                .collect()
        })
        .boxed()
}

fn table() -> BoxedStrategy<Block> {
    (1usize..=3, 1usize..=3)
        .prop_flat_map(|(cols, rows)| {
            (
                prop::collection::vec(
                    prop::option::of(prop::sample::select(
                        &[Alignment::Left, Alignment::Center, Alignment::Right][..],
                    )),
                    cols,
                ),
                prop::collection::vec(prop::collection::vec(run(false), cols), rows + 1),
            )
        })
        .prop_map(|(aligns, grid)| {
            let rows = grid
                .into_iter()
                .enumerate()
                .map(|(r, cells)| TableRow {
                    height: None,
                    cant_split: false,
                    cells: cells
                        .into_iter()
                        .zip(&aligns)
                        .map(|(inline, align)| TableCell {
                            properties: CellProperties::single(),
                            header: r == 0,
                            blocks: vec![Block::Paragraph(Paragraph {
                                properties: ParagraphProperties {
                                    alignment: *align,
                                    ..ParagraphProperties::none()
                                },
                                inlines: vec![inline],
                            })],
                        })
                        .collect(),
                })
                .collect();
            Block::Table(Table {
                properties: TableProperties {
                    style: None,
                    width: TableWidth::Auto,
                    alignment: None,
                    borders: None,
                    cell_padding: None,
                },
                column_widths: vec![Length::from_twips(1000); aligns.len()],
                rows,
            })
        })
        .boxed()
}

fn blocks() -> BoxedStrategy<Vec<Block>> {
    let one = prop_oneof![
        3 => inlines().prop_map(|i| vec![paragraph(Some("Normal"), i)]),
        1 => (1u8..=6, inlines()).prop_map(|(n, i)| vec![paragraph(Some(&format!("Heading{n}")), i)]),
        1 => inlines().prop_map(|i| vec![paragraph(Some("Quote"), i)]),
        2 => list(),
        1 => table().prop_map(|t| vec![t]),
    ];
    prop::collection::vec(one, 1..=5)
        .prop_map(|groups| groups.into_iter().flatten().collect())
        .boxed()
}

/// A document in Markdown's domain.
pub fn document() -> BoxedStrategy<Document> {
    blocks()
        .prop_map(|blocks| {
            let mut doc = Document::blank();
            doc.numbering = numbering();
            doc.sections[0].blocks = blocks;
            doc
        })
        .boxed()
}

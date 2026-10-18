//! The corpus's building blocks: headings, prose, bulleted and numbered lists, and tables with
//! a header row, on the built-in styles and two list definitions.

use super::words::{phrase, sentence, sentences};
use crate::prng::SplitMix64;
use cdoc_model::builtin::ids;
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Block, Border, BorderStyle, BorderWidth, Borders,
    CellProperties, Color, FirstLine, Indent, Inline, Length, Level, NumId, NumberFormat,
    Numbering, NumberingInstance, NumberingRef, Paragraph, ParagraphProperties, RunProperties,
    StyleId, Table, TableCell, TableProperties, TableRow, TableWidth,
};

/// The list instance bullets use.
pub const BULLETS: NumId = NumId(1);
/// The list instance numbered items use.
pub const NUMBERED: NumId = NumId(2);

/// The text width of the default A4 page (11 906 twips less two 1 440-twip margins).
const TEXT_WIDTH_TWIPS: i32 = 9_026;

fn paragraph(style: StyleId, numbering: Option<NumberingRef>, inlines: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style: Some(style),
            numbering,
            ..ParagraphProperties::none()
        },
        inlines,
    })
}

fn plain(text: String) -> Inline {
    Inline::text(text, RunProperties::none())
}

/// A heading of 3 to 7 title-case words at `level` (1 or 2).
pub fn heading(rng: &mut SplitMix64, level: u8) -> Block {
    let count = rng.between(3, 7);
    let style = ids::heading(level).unwrap_or_else(ids::normal);
    paragraph(style, None, vec![plain(phrase(rng, count, true))])
}

/// A body paragraph of 3 to 8 sentences; one in three has a bold sentence in the middle.
pub fn prose(rng: &mut SplitMix64) -> Block {
    let mut inlines = vec![plain(sentences(rng, 2, 4))];
    if rng.below(3) == 0 {
        let bold = RunProperties {
            bold: Some(true),
            ..RunProperties::none()
        };
        inlines.push(plain(" ".to_owned()));
        inlines.push(Inline::text(sentence(rng), bold));
    }
    inlines.push(plain(format!(" {}", sentences(rng, 1, 3))));
    paragraph(ids::normal(), None, inlines)
}

/// A list of 3 to 5 one-sentence items, some at the second level.
pub fn list(rng: &mut SplitMix64, num_id: NumId) -> Vec<Block> {
    let count = rng.between(3, 5);
    (0..count)
        .map(|k| {
            let level = u8::from(k > 0 && rng.below(3) == 0);
            let reference = NumberingRef { num_id, level };
            paragraph(
                ids::list_paragraph(),
                Some(reference),
                vec![plain(sentence(rng))],
            )
        })
        .collect()
}

fn cell(text: String, header: bool) -> TableCell {
    TableCell {
        properties: CellProperties::single(),
        header,
        blocks: vec![paragraph(ids::normal(), None, vec![plain(text)])],
    }
}

/// A full-width table: a header row of three titles, then 3 to 6 rows of short phrases.
pub fn table(rng: &mut SplitMix64) -> Block {
    let columns = 3;
    let header = TableRow {
        height: None,
        cant_split: true,
        cells: (0..columns)
            .map(|_| cell(phrase(rng, 2, true), true))
            .collect(),
    };
    let body_rows = rng.between(3, 6);
    let mut rows = vec![header];
    for _ in 0..body_rows {
        let cells = (0..columns)
            .map(|_| {
                let count = rng.between(1, 6);
                cell(phrase(rng, count, false), false)
            })
            .collect();
        rows.push(TableRow {
            height: None,
            cant_split: false,
            cells,
        });
    }
    let border = Border {
        style: BorderStyle::Single,
        width: BorderWidth(4),
        color: Some(Color::rgb(0, 0, 0)),
        space: Length(0),
    };
    Block::Table(Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Percent { fiftieths: 5000 },
            alignment: None,
            borders: Some(Borders::all(border)),
            cell_padding: Some(Length::from_twips(108)),
        },
        column_widths: vec![Length::from_twips(TEXT_WIDTH_TWIPS / 3); columns],
        rows,
    })
}

fn level(n: u8, format: NumberFormat, text: String) -> Level {
    Level {
        start: 1,
        format,
        text,
        alignment: Alignment::Left,
        indent: Some(Indent {
            left: Some(Length::from_twips(720 * (i32::from(n) + 1))),
            right: None,
            first_line: Some(FirstLine::Hanging {
                by: Length::from_twips(360),
            }),
        }),
        run: None,
    }
}

/// Two list definitions: bullets (`•`, `◦`) and decimal (`1.`, `1.1.`).
pub fn numbering() -> Numbering {
    let bullets = AbstractNumbering {
        id: AbstractNumId(1),
        levels: vec![
            level(0, NumberFormat::Bullet, "\u{2022}".to_owned()),
            level(1, NumberFormat::Bullet, "\u{25E6}".to_owned()),
        ],
    };
    let decimal = AbstractNumbering {
        id: AbstractNumId(2),
        levels: vec![
            level(0, NumberFormat::Decimal, "%1.".to_owned()),
            level(1, NumberFormat::Decimal, "%1.%2.".to_owned()),
        ],
    };
    let instance = |id: NumId, abstract_id: AbstractNumId| NumberingInstance {
        id,
        abstract_id,
        overrides: Vec::new(),
    };
    Numbering {
        definitions: vec![bullets, decimal],
        instances: vec![
            instance(BULLETS, AbstractNumId(1)),
            instance(NUMBERED, AbstractNumId(2)),
        ],
    }
}

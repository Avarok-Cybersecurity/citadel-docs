//! Benchmarks: writing and reading a 100-page document (an A4 page holds about six of these
//! 80-word paragraphs at 11pt): 600 paragraphs of mixed bold and italic runs, a three-item
//! list on every page and a 4x3 table every fifth page. Divide by 100 for time per page.

use cdoc_model::{
    Block, Document, Inline, InlineContent, Length, NumberingRef, Paragraph, ParagraphProperties,
    RunProperties, Table, TableCell, TableProperties, TableRow, TableWidth, TextRun,
};
use cdoc_odt::{read_odt, write_odt, ReadLimits, WriteOptions};
use cdoc_pkg::test_support::{library_for, IdFromBytes};
use criterion::Criterion;
use std::hint::black_box;

const PAGES: usize = 100;

fn run(text: String, bold: bool) -> Inline {
    let properties = RunProperties {
        bold: Some(bold),
        italic: Some(!bold),
        ..RunProperties::none()
    };
    Inline::plain(InlineContent::Text(TextRun { text, properties }))
}

fn paragraph(n: usize, numbering: Option<NumberingRef>) -> Block {
    let words: Vec<String> = (0..80)
        .map(|w| format!("word{}", (n * 31 + w * 7) % 997))
        .collect();
    let inlines = words
        .chunks(10)
        .enumerate()
        .map(|(i, c)| run(format!("{} ", c.join(" ")), i % 2 == 0))
        .collect();
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            numbering,
            ..ParagraphProperties::none()
        },
        inlines,
    })
}

fn table() -> Block {
    let cell = |t: &str| TableCell {
        properties: cdoc_model::CellProperties::single(),
        header: false,
        blocks: vec![Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines: vec![run(t.to_owned(), false)],
        })],
    };
    Block::Table(Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length(15_000); 3],
        rows: (0..4)
            .map(|r| TableRow {
                height: None,
                cant_split: false,
                cells: (0..3).map(|c| cell(&format!("r{r}c{c}"))).collect(),
            })
            .collect(),
    })
}

fn document() -> Document {
    let mut doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let list = doc.paragraphs().find_map(|p| p.properties.numbering);
    let mut blocks = Vec::new();
    for page in 0..PAGES {
        for k in 0..6 {
            blocks.push(paragraph(page * 6 + k, None));
        }
        for level in 0..3 {
            blocks.push(paragraph(
                page + level,
                list.map(|r| NumberingRef { level: 0, ..r }),
            ));
        }
        if page % 5 == 0 {
            blocks.push(table());
        }
    }
    doc.sections.truncate(1);
    doc.sections[0].blocks = blocks;
    doc.footnotes.clear();
    doc.comments.clear();
    doc
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    let doc = document();
    let media = library_for(&doc);
    let (bytes, _) = write_odt(&doc, &media, WriteOptions::document()).expect("write");
    let mut group = c.benchmark_group("odt_100_pages");
    group.sample_size(10);
    group.bench_function("write", |b| {
        b.iter(|| write_odt(black_box(&doc), &media, WriteOptions::document()).expect("write"));
    });
    group.bench_function("read", |b| {
        b.iter(|| {
            read_odt(black_box(&bytes), &ReadLimits::generous(), &mut IdFromBytes).expect("read")
        });
    });
    group.finish();
    c.final_summary();
}

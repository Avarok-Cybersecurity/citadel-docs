//! Merged cells: the model holds spans on the top-left cell, `.docx` holds `gridSpan` and
//! `vMerge` continuations. Tables are generated as complete grids partitioned into rectangles.

mod common;

use cdoc_model::Length;
use cdoc_model::{
    Block, CellProperties, Document, Inline, Paragraph, ParagraphProperties, RunProperties, Table,
    TableCell, TableProperties, TableRow, TableWidth,
};
use proptest::prelude::*;

fn cell(text: &str, col_span: u32, row_span: u32) -> TableCell {
    TableCell {
        properties: CellProperties {
            col_span,
            row_span,
            ..CellProperties::single()
        },
        header: false,
        blocks: vec![Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines: vec![Inline::text(text, RunProperties::none())],
        })],
    }
}

/// A full `rows` x `cols` grid cut into rectangles, one choice pair per free slot.
fn grid(rows: usize, cols: usize, choices: &[(u32, u32)]) -> Table {
    let mut taken = vec![vec![false; cols]; rows];
    let mut table_rows: Vec<TableRow> = Vec::new();
    let mut pick = choices.iter().cycle();
    for r in 0..rows {
        let mut cells = Vec::new();
        for c in 0..cols {
            if taken[r][c] {
                continue;
            }
            let &(wide, tall) = pick.next().expect("cycle");
            let free_right = (c..cols).take_while(|&k| !taken[r][k]).count();
            let w = (wide as usize % free_right) + 1;
            let free_down = (r..rows)
                .take_while(|&k| (c..c + w).all(|j| !taken[k][j]))
                .count();
            let h = (tall as usize % free_down) + 1;
            for row in taken.iter_mut().skip(r).take(h) {
                for slot in row.iter_mut().skip(c).take(w) {
                    *slot = true;
                }
            }
            cells.push(cell(
                &format!("r{r}c{c}"),
                u32::try_from(w).unwrap(),
                u32::try_from(h).unwrap(),
            ));
        }
        table_rows.push(TableRow {
            height: None,
            cant_split: false,
            cells,
        });
    }
    Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length::from_twips(1500); cols],
        rows: table_rows,
    }
}

proptest! {
    #[test]
    fn complete_merged_grids_round_trip(
        rows in 1usize..=5,
        cols in 1usize..=5,
        choices in prop::collection::vec((0u32..4, 0u32..4), 1..30),
    ) {
        let mut doc = Document::blank();
        doc.sections[0].blocks = vec![Block::Table(grid(rows, cols, &choices))];
        let (back, written, _) = common::round_trip(&doc);
        prop_assert!(back == doc, "{}", common::first_diff(&back, &doc).unwrap_or_default());
        prop_assert!(written.is_lossless(), "{:?}", written);
    }
}

#[test]
fn a_merge_past_the_last_row_is_reported_not_silent() {
    let mut doc = Document::blank();
    let mut table = grid(1, 2, &[(0, 0)]);
    table.rows[0].cells[0].properties.row_span = 3;
    doc.sections[0].blocks = vec![Block::Table(table)];
    let (_, written) = common::write(&doc);
    assert_eq!(
        written.count_of("a merged cell extends past the last row"),
        1
    );
}

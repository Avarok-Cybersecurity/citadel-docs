//! `Table::grid` unpacks spans into a rectangle.

use cdoc_model::{
    Block, CellProperties, Length, Paragraph, ParagraphProperties, Slot, Table, TableCell,
    TableProperties, TableRow, TableWidth,
};

fn cell(col_span: u32, row_span: u32) -> TableCell {
    TableCell {
        properties: CellProperties {
            col_span,
            row_span,
            ..CellProperties::single()
        },
        header: false,
        blocks: vec![Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines: Vec::new(),
        })],
    }
}

fn table(cols: usize, rows: Vec<Vec<TableCell>>) -> Table {
    Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment: None,
            borders: None,
            cell_padding: None,
        },
        column_widths: vec![Length(100); cols],
        rows: rows
            .into_iter()
            .map(|cells| TableRow {
                height: None,
                cant_split: false,
                cells,
            })
            .collect(),
    }
}

fn shape(t: &Table) -> Vec<String> {
    t.grid()
        .iter()
        .map(|row| {
            row.iter()
                .map(|s| match s {
                    Slot::Cell(_) => 'C',
                    Slot::Covered => 'x',
                    Slot::Empty => '.',
                })
                .collect()
        })
        .collect()
}

#[test]
fn spans_cover_the_positions_they_span() {
    // [  A  ][B]
    // [C][D ][B]  (B spans two rows; D is pushed right of C)
    let t = table(
        3,
        vec![vec![cell(2, 1), cell(1, 2)], vec![cell(1, 1), cell(1, 1)]],
    );
    assert_eq!(shape(&t), ["CxC", "CCx"]);
}

#[test]
fn ragged_rows_are_padded_with_empty_positions() {
    let t = table(
        3,
        vec![vec![cell(1, 1)], vec![cell(1, 1), cell(1, 1), cell(1, 1)]],
    );
    assert_eq!(shape(&t), ["C..", "CCC"]);
}

#[test]
fn a_span_wider_than_the_grid_is_clamped() {
    let t = table(2, vec![vec![cell(5, 1)]]);
    assert_eq!(shape(&t), ["Cx"]);
}

#[test]
fn a_span_ends_after_its_rows() {
    // The tall cell covers one position in the second row only; the third row is free again.
    let t = table(
        2,
        vec![
            vec![cell(1, 2), cell(1, 1)],
            vec![cell(1, 1)],
            vec![cell(1, 1), cell(1, 1)],
        ],
    );
    assert_eq!(shape(&t), ["CC", "xC", "CC"]);
}

#[test]
fn placements_give_each_cell_its_position_and_clamped_spans() {
    let t = table(
        3,
        vec![vec![cell(2, 5), cell(1, 1)], vec![cell(1, 1), cell(1, 1)]],
    );
    let got: Vec<(usize, usize, usize, usize, usize)> = t
        .placements()
        .iter()
        .map(|p| (p.row, p.index, p.col, p.col_span, p.row_span))
        .collect();
    // the tall cell's span is clamped to the two rows; row 1 starts under it, at column 2
    assert_eq!(got, [(0, 0, 0, 2, 2), (0, 1, 2, 1, 1), (1, 0, 2, 1, 1)]);
}

#[test]
fn a_cell_starting_past_the_last_column_is_left_out() {
    let t = table(1, vec![vec![cell(1, 1), cell(1, 1)]]);
    assert_eq!(t.placements().len(), 1);
}

#[test]
fn placements_clamp_a_span_to_the_grid() {
    let t = table(2, vec![vec![cell(5, 1)]]);
    assert_eq!(t.placements()[0].col_span, 2);
}

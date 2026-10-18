//! Tables: insertion, rows and columns, merges and spans, deletion. The model each command
//! leaves is the one Word would, and stays valid.

mod common;

use cdoc_edit::{Command, Selection};
use cdoc_model::{Block, InlineContent, Table};
use common::*;

fn table(e: &cdoc_edit::Editor<'_>, block: usize) -> Table {
    match &e.model().sections[0].blocks[block] {
        Block::Table(t) => t.clone(),
        Block::Paragraph(_) => panic!("block {block} is a paragraph"),
    }
}

/// A 2x3 table between "ab" and "cd", the caret in its first cell.
fn with_table() -> cdoc_edit::Editor<'static> {
    let mut e = editor(&paragraphs(&["abcd"]), 1);
    let g = gap(&e, 0, 2);
    caret(&mut e, g);
    e.apply(Command::InsertTable { rows: 2, cols: 3 }).unwrap();
    check(&e);
    e
}

fn cell_text(t: &Table, row: usize, cell: usize) -> String {
    t.rows[row].cells[cell]
        .blocks
        .iter()
        .flat_map(Block::paragraphs)
        .flat_map(|p| &p.inlines)
        .map(|i| match &i.content {
            InlineContent::Text(r) => r.text.clone(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn a_table_splits_the_paragraph_and_takes_the_caret() {
    let mut e = with_table();
    assert_eq!(
        (text(&e, 0), text(&e, 2)),
        ("ab".to_owned(), "cd".to_owned())
    );
    let t = table(&e, 1);
    assert_eq!(
        (t.rows.len(), t.rows[0].cells.len(), t.column_widths.len()),
        (2, 3, 3)
    );
    type_chars(&mut e, "x");
    check(&e);
    assert_eq!(cell_text(&table(&e, 1), 0, 0), "x");
}

#[test]
fn rows_and_columns_go_where_asked() {
    let mut e = with_table();
    type_chars(&mut e, "A");
    e.apply(Command::InsertRowAbove).unwrap();
    e.apply(Command::InsertColumnRight).unwrap();
    check(&e);
    let t = table(&e, 1);
    assert_eq!((t.rows.len(), t.column_widths.len()), (3, 4));
    assert_eq!(cell_text(&t, 1, 0), "A", "the caret row moved down by one");
    assert_eq!(
        t.rows.iter().map(|r| r.cells.len()).collect::<Vec<_>>(),
        vec![4, 4, 4]
    );
    e.apply(Command::DeleteColumn).unwrap();
    e.apply(Command::DeleteRow).unwrap();
    check(&e);
    let t = table(&e, 1);
    assert_eq!((t.rows.len(), t.column_widths.len()), (2, 3));
    assert_eq!(cell_text(&t, 0, 0), "", "the row holding A is gone");
}

/// The gap `offset` into the first paragraph of cell `(row, cell)` of the table at `block`.
fn in_cell(
    e: &cdoc_edit::Editor<'_>,
    block: usize,
    row: usize,
    cell: usize,
    offset: usize,
) -> cdoc_edit::Position {
    let path = cdoc_edit::ModelPath {
        section: 0,
        block,
        cells: vec![(row, cell, 0)],
        offset,
    };
    e.position_at_path(&path).unwrap()
}

#[test]
fn merging_cells_spans_them_and_keeps_their_text() {
    let mut e = with_table();
    type_chars(&mut e, "1");
    let p = in_cell(&e, 1, 0, 1, 0);
    e.apply(Command::SetSelection(Selection::caret(p))).unwrap();
    type_chars(&mut e, "2");
    let s = Selection {
        anchor: in_cell(&e, 1, 0, 0, 0),
        head: in_cell(&e, 1, 0, 1, 1),
    };
    e.apply(Command::SetSelection(s)).unwrap();
    e.apply(Command::MergeCells).unwrap();
    check(&e);
    let t = table(&e, 1);
    assert_eq!(t.rows[0].cells.len(), 2);
    assert_eq!(t.rows[0].cells[0].properties.col_span, 2);
    assert_eq!(cell_text(&t, 0, 0), "12");
    e.apply(Command::InsertRowBelow).unwrap();
    e.apply(Command::InsertColumnLeft).unwrap();
    check(&e);
    let t = table(&e, 1);
    assert_eq!(t.column_widths.len(), 4);
    assert_eq!(
        t.rows[0].cells[1].properties.col_span, 2,
        "the merged cell keeps its span"
    );
}

#[test]
fn a_row_inserted_through_a_vertical_merge_extends_it() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    e.apply(Command::InsertTable { rows: 3, cols: 2 }).unwrap();
    let s = Selection {
        anchor: in_cell(&e, 0, 0, 0, 0),
        head: in_cell(&e, 0, 1, 0, 0),
    };
    e.apply(Command::SetSelection(s)).unwrap();
    e.apply(Command::MergeCells).unwrap();
    check(&e);
    assert_eq!(table(&e, 0).rows[0].cells[0].properties.row_span, 2);
    let p = in_cell(&e, 0, 0, 1, 0);
    e.apply(Command::SetSelection(Selection::caret(p))).unwrap();
    e.apply(Command::InsertRowBelow).unwrap();
    check(&e);
    let t = table(&e, 0);
    assert_eq!(
        (t.rows.len(), t.rows[0].cells[0].properties.row_span),
        (4, 3)
    );
    e.apply(Command::DeleteRow).unwrap();
    check(&e);
    assert_eq!(table(&e, 0).rows[0].cells[0].properties.row_span, 2);
}

#[test]
fn merging_a_column_of_whole_rows_removes_the_emptied_rows() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    e.apply(Command::InsertTable { rows: 3, cols: 1 }).unwrap();
    let s = Selection {
        anchor: in_cell(&e, 0, 0, 0, 0),
        head: in_cell(&e, 0, 2, 0, 0),
    };
    e.apply(Command::SetSelection(s)).unwrap();
    e.apply(Command::MergeCells).unwrap();
    check(&e);
    let t = table(&e, 0);
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].cells[0].properties.row_span, 1);
}

#[test]
fn deleting_the_table_leaves_a_paragraph() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    let g = gap(&e, 0, 0);
    caret(&mut e, g);
    e.apply(Command::InsertTable { rows: 1, cols: 1 }).unwrap();
    e.apply(Command::DeleteTable).unwrap();
    check(&e);
    assert!(e.model().sections[0]
        .blocks
        .iter()
        .all(|b| matches!(b, Block::Paragraph(_))));
}

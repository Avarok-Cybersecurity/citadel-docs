//! Random command sequences on the kitchen sink and on plain paragraphs: after every command,
//! undo and redo, the model view equals the replica's model and the document validates.
//! Commands may be refused where they do not apply; nothing else may fail.

mod common;

use cdoc_edit::Command;
use common::ops::{apply, op, Op};
use common::{check, editor, paragraphs};
use proptest::prelude::*;

fn run(start: &cdoc_model::Document, ops: &[Op]) {
    let mut e = editor(start, 1);
    for (serial, op) in ops.iter().enumerate() {
        let _ = apply(&mut e, op, serial);
        check(&e);
    }
}

/// Paragraphs around a 3x3 table, so most random selections land in cells.
fn table_doc() -> cdoc_model::Document {
    let mut e = editor(&paragraphs(&["before", "after"]), 9);
    e.apply(Command::InsertTable { rows: 3, cols: 3 })
        .expect("inserts");
    e.apply(Command::InsertText("cell".to_owned()))
        .expect("types");
    e.model().clone()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn commands_keep_the_kitchen_sink_valid(ops in proptest::collection::vec(op(), 1..24)) {
        run(&cdoc_model::test_support::fixtures::kitchen_sink(), &ops);
    }

    #[test]
    fn commands_keep_tables_valid(ops in proptest::collection::vec(op(), 1..32)) {
        run(&table_doc(), &ops);
    }

    #[test]
    fn commands_keep_plain_paragraphs_valid(ops in proptest::collection::vec(op(), 1..32)) {
        run(&paragraphs(&["alpha beta gamma", "", "delta"]), &ops);
    }
}

//! The mutable walkers reach every paragraph and table wherever it lives.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::visit_mut::{paragraphs_in, tables_in};
use cdoc_model::{Block, Inline, RunProperties};

#[test]
fn every_paragraph_is_visited_exactly_once() {
    let mut doc = kitchen_sink();
    let expected = doc.paragraphs().count();
    let mut visited = 0;
    doc.for_each_paragraph_mut(&mut |p| {
        visited += 1;
        p.inlines.push(Inline::text("#", RunProperties::none()));
    });
    assert_eq!(visited, expected);
    assert!(doc.paragraphs().all(|p| matches!(
        p.inlines.last().map(|i| &i.content),
        Some(cdoc_model::InlineContent::Text(t)) if t.text == "#"
    )));
}

#[test]
fn containers_cover_bodies_headers_footers_and_footnotes() {
    let mut doc = kitchen_sink();
    let mut lists = 0;
    doc.for_each_container_mut(&mut |_| lists += 1);
    // two section bodies, four headers and one footer in the first section, the same four and
    // one again in the second (it copies the first's properties), and one footnote
    assert_eq!(lists, 2 + 2 * (3 + 1) + 1);
}

#[test]
fn tables_are_visited_outermost_first_including_nested() {
    let mut doc = kitchen_sink();
    let mut widths = Vec::new();
    doc.for_each_table_mut(&mut |t| widths.push(t.column_widths.len()));
    assert_eq!(
        widths,
        [2, 1],
        "the outer table, then the one nested in its cell"
    );
}

#[test]
fn free_walkers_work_on_a_block_list() {
    let mut blocks: Vec<Block> = kitchen_sink().sections.remove(0).blocks;
    let mut paragraphs = 0;
    paragraphs_in(&mut blocks, &mut |_| paragraphs += 1);
    let mut tables = 0;
    tables_in(&mut blocks, &mut |_| tables += 1);
    // title, heading, rich paragraph, list item, three in the tables, quote
    assert_eq!((paragraphs, tables), (8, 2));
}

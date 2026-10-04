//! The checked-in `schema/prosemirror-schema.json` is exactly what the crate generates. The UI
//! reads that file (and `cdoc-wasm`'s `schema_json()`) to check its TipTap schema, so a drift
//! between the file and the code would let the two sides disagree silently.

use cdoc_pm::schema::{schema, schema_json};

const CHECKED_IN: &str = include_str!("../../../schema/prosemirror-schema.json");

#[test]
fn checked_in_schema_matches_the_generated_one() {
    assert_eq!(
        CHECKED_IN.trim_end(),
        schema_json(),
        "schema/prosemirror-schema.json is stale; regenerate with \
         `cargo run -p cdoc-pm --example emit_schema > schema/prosemirror-schema.json`"
    );
}

#[test]
fn schema_names_are_unique_and_every_node_child_is_declared() {
    let spec = schema();
    let mut names: Vec<&str> = spec.nodes.iter().map(|n| n.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), spec.nodes.len(), "duplicate node name");
    let mut marks: Vec<&str> = spec.marks.iter().map(|m| m.name.as_str()).collect();
    marks.sort_unstable();
    marks.dedup();
    assert_eq!(marks.len(), spec.marks.len(), "duplicate mark name");
    for node in &spec.nodes {
        for child in &node.children {
            assert!(
                spec.node(child).is_some(),
                "{child} under {} is undeclared",
                node.name
            );
        }
        if node.marks != "_" && !node.marks.is_empty() {
            for mark in node.marks.split(' ') {
                assert!(
                    spec.mark(mark).is_some(),
                    "{mark} on {} is undeclared",
                    node.name
                );
            }
        }
    }
    assert_eq!(spec.top_node, "doc");
}

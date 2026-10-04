//! Malformed ProseMirror JSON is rejected with a path, never accepted silently.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_pm::{from_pm, from_pm_json, to_pm, PmMark, PmNode};
use serde_json::{json, Map};

fn kitchen_pm() -> PmNode {
    to_pm(&kitchen_sink())
}

fn err(node: &PmNode) -> String {
    from_pm(node).expect_err("must be rejected").to_string()
}

#[test]
fn wrong_root_and_unknown_child() {
    let mut pm = kitchen_pm();
    pm.kind = "paragraph".to_owned();
    assert!(err(&pm).contains("expected a \"doc\" node"));

    let mut pm = kitchen_pm();
    pm.content.insert(0, PmNode::new("bulletList"));
    let e = err(&pm);
    assert!(e.starts_with("doc/bulletList[0]"), "{e}");
    assert!(e.contains("not allowed in doc"));
}

#[test]
fn missing_document_attribute() {
    let mut pm = kitchen_pm();
    pm.attrs.remove("styles");
    assert!(err(&pm).contains("missing attribute \"styles\""));
}

#[test]
fn unknown_block_and_unknown_inline_name_their_path() {
    let mut pm = kitchen_pm();
    pm.content[0].content.push(PmNode::new("codeBlock"));
    let e = err(&pm);
    assert!(
        e.contains("codeBlock") && e.contains("is not a block"),
        "{e}"
    );

    let mut pm = kitchen_pm();
    pm.content[0].content[4]
        .content
        .push(PmNode::new("mention"));
    let e = err(&pm);
    assert!(
        e.starts_with("doc/section[0]/paragraph[0]/mention[1]"),
        "{e}"
    );
}

#[test]
fn text_without_text_and_formatting_marks_on_an_image() {
    let mut pm = kitchen_pm();
    let paragraph = &mut pm.content[0].content[4];
    paragraph.content[0].text = None;
    assert!(err(&pm).contains("text node without text"));

    let mut pm = kitchen_pm();
    let mut image = PmNode::new("image");
    image.attrs =
        serde_json::from_value(json!({"contentId": "x", "width": 1, "height": 1})).unwrap();
    let mut on = Map::new();
    on.insert("on".to_owned(), json!(true));
    image.marks.push(PmMark::new("bold", on));
    pm.content[0].content[4].content.push(image);
    assert!(err(&pm).contains("character formatting marks on a \"image\" node"));
}

#[test]
fn unknown_duplicate_and_conflicting_marks() {
    let text = |marks: Vec<PmMark>| {
        let mut pm = kitchen_pm();
        let mut t = PmNode::new("text");
        t.text = Some("x".to_owned());
        t.marks = marks;
        pm.content[0].content[4].content.push(t);
        err(&pm)
    };
    let on = |v: bool| {
        let mut m = Map::new();
        m.insert("on".to_owned(), json!(v));
        m
    };
    assert!(text(vec![PmMark::new("code", Map::new())]).contains("unknown mark \"code\""));
    assert!(text(vec![
        PmMark::new("bold", on(true)),
        PmMark::new("bold", on(false))
    ])
    .contains("duplicate mark \"bold\""));
    let rev = |kind: &str| {
        PmMark::new(
            kind,
            serde_json::from_value(json!({"id": "r", "author": "a", "date": "d"})).unwrap(),
        )
    };
    assert!(
        text(vec![rev("insertion"), rev("deletion")]).contains("both insertion and deletion marks")
    );
    assert!(text(vec![PmMark::new("bold", Map::new())]).contains("missing attribute \"on\""));
}

#[test]
fn bad_attribute_values_are_reported_with_the_path() {
    let mut pm = kitchen_pm();
    pm.content[0].content[4]
        .attrs
        .insert("alignment".to_owned(), json!("middle"));
    let e = err(&pm);
    assert!(
        e.starts_with("doc/section[0]/paragraph[0]: bad attributes"),
        "{e}"
    );

    let mut pm = kitchen_pm();
    pm.content[0].content[8].content[0].content[0]
        .attrs
        .insert("colspan".to_owned(), json!(-1));
    let e = err(&pm);
    assert!(e.contains("bad attribute \"colspan\""), "{e}");
}

#[test]
fn duplicate_header_kind_and_second_footnotes_node() {
    let mut pm = kitchen_pm();
    let header = pm.content[0].content[0].clone();
    pm.content[0].content.insert(1, header);
    assert!(err(&pm).contains("duplicate header of kind Default"));

    let mut pm = kitchen_pm();
    let footnotes = pm.content.last().unwrap().clone();
    pm.content.push(footnotes);
    assert!(err(&pm).contains("more than one footnotes node"));
}

#[test]
fn null_attributes_read_as_absent_and_unparseable_text_is_an_error() {
    // ProseMirror fills every undeclared-but-schema'd attribute with null; that must decode
    // as "unset", not fail.
    let mut pm = kitchen_pm();
    pm.content[0].content[4]
        .attrs
        .insert("alignment".to_owned(), serde_json::Value::Null);
    assert_eq!(from_pm(&pm).unwrap(), kitchen_sink());
    assert!(from_pm_json("{not json")
        .unwrap_err()
        .to_string()
        .contains("not a ProseMirror node"));
}

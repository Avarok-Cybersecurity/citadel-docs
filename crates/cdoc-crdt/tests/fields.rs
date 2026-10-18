//! Block properties merge field by field and comments merge comment by comment: concurrent
//! edits of different fields (or different comments) all survive, on every replica.

mod common;

use cdoc_crdt::{from_model, to_model, Atom, ClientId, Doc, DocAttr, EditError};
use cdoc_model::{
    Alignment, Block, Comment, CommentId, Document, Length, ParagraphProperties, TableProperties,
    TableWidth,
};
use common::ROOM;

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

fn exchange(a: &mut Doc, b: &mut Doc) {
    let (to_b, to_a) = (
        a.encode_diff(&b.state_vector()),
        b.encode_diff(&a.state_vector()),
    );
    b.apply_update(&to_b, ROOM).unwrap();
    a.apply_update(&to_a, ROOM).unwrap();
    assert_eq!(a.encode_snapshot(), b.encode_snapshot());
}

fn paragraph(d: &Doc) -> ParagraphProperties {
    let Block::Paragraph(p) = &to_model(d).unwrap().sections[0].blocks[0] else {
        unreachable!("the blank document is one paragraph")
    };
    p.properties.clone()
}

#[test]
fn alignment_here_and_indent_there_both_survive() {
    let base = from_model(&Document::blank(), ClientId(100)).unwrap();
    let (mut a, mut b) = (replica(&base, 1), replica(&base, 2));
    let centred = ParagraphProperties {
        alignment: Some(Alignment::Center),
        ..paragraph(&a)
    };
    a.set_block(1, &Atom::Paragraph(centred)).unwrap();
    let kept = ParagraphProperties {
        keep_with_next: Some(true),
        outline_level: Some(2),
        ..paragraph(&b)
    };
    b.set_block(1, &Atom::Paragraph(kept)).unwrap();
    exchange(&mut a, &mut b);
    let p = paragraph(&a);
    assert_eq!(p.alignment, Some(Alignment::Center));
    assert_eq!((p.keep_with_next, p.outline_level), (Some(true), Some(2)));
    // Clearing a field is an edit of that field alone.
    let cleared = ParagraphProperties {
        alignment: None,
        ..paragraph(&b)
    };
    b.set_block(1, &Atom::Paragraph(cleared)).unwrap();
    exchange(&mut a, &mut b);
    let p = paragraph(&a);
    assert_eq!(p.alignment, None);
    assert_eq!((p.keep_with_next, p.outline_level), (Some(true), Some(2)));
}

#[test]
fn setting_unchanged_properties_writes_nothing() {
    let mut d = from_model(&Document::blank(), ClientId(1)).unwrap();
    let sv = d.state_vector();
    d.set_block(1, &Atom::Paragraph(paragraph(&d))).unwrap();
    assert_eq!(d.state_vector(), sv);
}

#[test]
fn table_widths_here_and_alignment_there_both_survive() {
    let mut base = Doc::new(ClientId(100));
    let table = |widths: Vec<Length>, alignment| Atom::Table {
        properties: TableProperties {
            style: None,
            width: TableWidth::Auto,
            alignment,
            borders: None,
            cell_padding: None,
        },
        column_widths: widths,
    };
    base.insert_atom(0, &table(vec![Length(100)], None))
        .unwrap();
    let (mut a, mut b) = (replica(&base, 1), replica(&base, 2));
    a.set_block(0, &table(vec![Length(100), Length(200)], None))
        .unwrap();
    b.set_block(0, &table(vec![Length(100)], Some(Alignment::Right)))
        .unwrap();
    exchange(&mut a, &mut b);
    // Read back through a model-free probe: a third write of the merged value changes nothing.
    let merged = table(vec![Length(100), Length(200)], Some(Alignment::Right));
    let sv = a.state_vector();
    a.set_block(0, &merged).unwrap();
    assert_eq!(a.state_vector(), sv, "the merge holds both fields");
}

fn comment(id: &str, text: &str, resolved: bool) -> Comment {
    Comment {
        id: CommentId::new(id),
        author: "A".to_owned(),
        date: "2026-10-17T00:00:00Z".to_owned(),
        text: text.to_owned(),
        resolved,
        parent: None,
    }
}

fn comments(d: &Doc) -> Vec<Comment> {
    to_model(d).unwrap().comments
}

#[test]
fn concurrent_changes_to_different_comments_all_survive_in_order() {
    let mut doc = Document::blank();
    doc.comments = vec![comment("c1", "one", false), comment("c2", "two", false)];
    let base = from_model(&doc, ClientId(100)).unwrap();
    let (mut a, mut b) = (replica(&base, 1), replica(&base, 2));
    let mut c = replica(&base, 3);
    // a resolves c1, b rewrites c2, c adds c3 and removes nothing.
    a.set_attr(&DocAttr::Comments(vec![
        comment("c1", "one", true),
        comment("c2", "two", false),
    ]))
    .unwrap();
    b.set_attr(&DocAttr::Comments(vec![
        comment("c1", "one", false),
        comment("c2", "2!", false),
    ]))
    .unwrap();
    let mut with_c3 = comments(&c);
    with_c3.push(comment("c3", "three", false));
    c.set_attr(&DocAttr::Comments(with_c3)).unwrap();
    exchange(&mut a, &mut b);
    exchange(&mut a, &mut c);
    exchange(&mut b, &mut c);
    let expected = vec![
        comment("c1", "one", true),
        comment("c2", "2!", false),
        comment("c3", "three", false),
    ];
    assert_eq!(comments(&a), expected);
    // Removing one comment while another replica edits a different one.
    a.set_attr(&DocAttr::Comments(expected[1..].to_vec()))
        .unwrap();
    b.set_attr(&DocAttr::Comments(vec![
        expected[0].clone(),
        expected[1].clone(),
        comment("c3", "three", true),
    ]))
    .unwrap();
    exchange(&mut a, &mut b);
    assert_eq!(
        comments(&b),
        vec![comment("c2", "2!", false), comment("c3", "three", true)]
    );
}

#[test]
fn a_comment_list_naming_one_id_twice_is_refused() {
    let mut d = from_model(&Document::blank(), ClientId(1)).unwrap();
    let twice = vec![comment("c1", "a", false), comment("c1", "b", false)];
    assert_eq!(
        d.set_attr(&DocAttr::Comments(twice)),
        Err(EditError::DuplicateId {
            element: "comment",
            id: format!("{:?}", CommentId::new("c1"))
        })
    );
}

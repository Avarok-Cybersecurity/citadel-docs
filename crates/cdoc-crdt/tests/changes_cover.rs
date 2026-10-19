//! Property: `Changes` covers every block whose model changed. Replicas edit text, paragraph
//! boundaries, formatting, per-field block properties, comments and metadata concurrently;
//! each time one applies an update, the blocks of its model that `changes.blocks` does not name
//! must be, in order, blocks the model had before (a subsequence of the old blocks): a block
//! that changed and is not reported cannot be found there. A changed attribute must be named
//! in `changes.attrs`.
//! Edits keep the sequence well formed (unit 1, the first paragraph, is never deleted), so the
//! model's blocks are the sequence's blocks one for one.

mod common;

use cdoc_crdt::{from_model, to_model, Atom, Changes, ClientId, Doc, DocAttr, Mark};
use cdoc_model::{Alignment, Block, Comment, CommentId, Document, DocumentMeta, Inline};
use cdoc_model::{Paragraph, ParagraphProperties, RunProperties};
use common::ROOM;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Edit {
    Text(String),
    Delete(u8),
    Paragraph,
    Bold(u8),
    Align(bool),
    Keep(bool),
    Comment(u8),
    Title(u8),
}

fn edit() -> BoxedStrategy<Edit> {
    prop_oneof![
        4 => "[a-c]{1,3}".prop_map(Edit::Text),
        2 => (1u8..5).prop_map(Edit::Delete),
        1 => Just(Edit::Paragraph),
        2 => (1u8..6).prop_map(Edit::Bold),
        1 => any::<bool>().prop_map(Edit::Align),
        1 => any::<bool>().prop_map(Edit::Keep),
        1 => any::<u8>().prop_map(Edit::Comment),
        1 => any::<u8>().prop_map(Edit::Title),
    ]
    .boxed()
}

#[derive(Debug, Clone)]
enum Step {
    Edit(usize, u16, Edit),
    Deliver(u16, usize),
}

fn steps() -> BoxedStrategy<Vec<Step>> {
    let step = prop_oneof![
        3 => (0..3usize, any::<u16>(), edit()).prop_map(|(r, at, e)| Step::Edit(r, at, e)),
        2 => (any::<u16>(), 0..3usize).prop_map(|(m, to)| Step::Deliver(m, to)),
    ];
    prop::collection::vec(step, 1..60).boxed()
}

fn apply(d: &mut Doc, at: u16, edit: &Edit) {
    let len = d.len();
    let pos = 2 + usize::from(at) * (len - 1) / 65_536;
    let paragraphs: Vec<usize> = d
        .visible_text()
        .chars()
        .enumerate()
        .filter(|&(i, c)| i > 0 && c == '\u{fffc}')
        .map(|(i, _)| i)
        .collect();
    let paragraph_at = paragraphs[usize::from(at) % paragraphs.len()];
    let model = to_model(d).unwrap();
    let props = |k: usize| match &model.sections[0].blocks[k] {
        Block::Paragraph(p) => p.properties.clone(),
        Block::Table(_) => unreachable!("only paragraphs"),
    };
    let k = paragraphs.iter().position(|&p| p == paragraph_at).unwrap();
    match edit {
        Edit::Text(t) => d.insert_text(pos, t).unwrap(),
        Edit::Delete(n) if pos < len => d.delete(pos, usize::from(*n).min(len - pos)).unwrap(),
        Edit::Paragraph => d
            .insert_atom(pos, &Atom::Paragraph(ParagraphProperties::none()))
            .unwrap(),
        Edit::Bold(n) if pos < len => {
            let bold = RunProperties {
                bold: Some(true),
                ..RunProperties::none()
            };
            d.format(pos..(pos + usize::from(*n)).min(len), &Mark::Run(bold))
                .unwrap();
        }
        Edit::Align(on) => {
            let p = ParagraphProperties {
                alignment: on.then_some(Alignment::Center),
                ..props(k)
            };
            d.set_block(paragraph_at, &Atom::Paragraph(p)).unwrap();
        }
        Edit::Keep(on) => {
            let p = ParagraphProperties {
                keep_with_next: Some(*on),
                ..props(k)
            };
            d.set_block(paragraph_at, &Atom::Paragraph(p)).unwrap();
        }
        Edit::Comment(v) => {
            let mut list = model.comments.clone();
            let id = format!("c{}", v % 3);
            match list.iter().position(|c| c.id.as_str() == id) {
                Some(i) if v % 2 == 0 => {
                    list.remove(i);
                }
                Some(i) => list[i].resolved = !list[i].resolved,
                None => list.push(Comment {
                    id: CommentId::new(id),
                    author: "A".to_owned(),
                    date: "2026-10-17T00:00:00Z".to_owned(),
                    text: "t".to_owned(),
                    resolved: false,
                    parent: None,
                }),
            }
            d.set_attr(&DocAttr::Comments(list)).unwrap();
        }
        Edit::Title(v) => d
            .set_attr(&DocAttr::Meta(DocumentMeta {
                title: Some(format!("t{}", v % 4)),
                language: None,
            }))
            .unwrap(),
        Edit::Delete(_) | Edit::Bold(_) => {}
    }
}

/// The section, then each paragraph, as comparable text.
fn blocks(d: &Doc) -> (Vec<String>, String, String) {
    let m = to_model(d).unwrap();
    let mut out = vec![format!("{:?}", m.sections[0].properties)];
    out.extend(m.sections[0].blocks.iter().map(|b| format!("{b:?}")));
    (out, format!("{:?}", m.meta), format!("{:?}", m.comments))
}

/// Whether `kept`, in order, is a subsequence of `before`.
fn subsequence(kept: &[&String], before: &[String]) -> bool {
    let mut old = before.iter();
    kept.iter().all(|k| old.any(|b| b == *k))
}

fn check(
    before: &(Vec<String>, String, String),
    after: &(Vec<String>, String, String),
    c: &Changes,
) -> Result<(), TestCaseError> {
    let kept: Vec<&String> = after
        .0
        .iter()
        .enumerate()
        .filter(|(b, _)| !c.blocks.iter().any(|r| r.contains(b)))
        .map(|(_, text)| text)
        .collect();
    prop_assert!(
        subsequence(&kept, &before.0),
        "a block outside {:?} changed",
        c.blocks
    );
    if before.1 != after.1 {
        prop_assert!(
            c.attrs.iter().any(|a| a == "meta"),
            "meta changed: {:?}",
            c.attrs
        );
    }
    if before.2 != after.2 {
        prop_assert!(
            c.attrs.iter().any(|a| a == "comments"),
            "comments changed: {:?}",
            c.attrs
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn changes_cover_every_block_whose_model_changed(steps in steps()) {
        let mut doc = Document::blank();
        doc.sections[0].blocks = (0..4)
            .map(|_| Block::Paragraph(Paragraph {
                properties: ParagraphProperties::none(),
                inlines: vec![Inline::text("ab", RunProperties::none())],
            }))
            .collect();
        let base = from_model(&doc, ClientId(100)).unwrap().encode_snapshot();
        let mut docs: Vec<Doc> = (1..=3).map(|c| Doc::decode_snapshot(&base, ClientId(c)).unwrap()).collect();
        let mut messages: Vec<Vec<u8>> = Vec::new();
        for s in &steps {
            match s {
                Step::Edit(r, at, e) => {
                    let sv = docs[*r].state_vector();
                    apply(&mut docs[*r], *at, e);
                    messages.push(docs[*r].encode_diff(&sv));
                }
                Step::Deliver(m, to) if !messages.is_empty() => {
                    let before = blocks(&docs[*to]);
                    let applied = docs[*to].apply_update(&messages[usize::from(*m) % messages.len()], ROOM).unwrap();
                    check(&before, &blocks(&docs[*to]), &applied.changes)?;
                }
                Step::Deliver(..) => {}
            }
        }
    }
}

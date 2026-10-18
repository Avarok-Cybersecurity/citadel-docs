//! Property: 2 to 5 replicas edit block-property fields and comments concurrently, each
//! (paragraph, field) and each comment owned by at most one replica; updates arrive in
//! arbitrary orders. Every replica converges, every owner's last value of its fields and
//! comments survives (nothing is lost to another replica's edit of a different field), and a
//! replica's own edit reads back through `to_model` exactly as it was set.

use cdoc_crdt::{from_model, to_model, Atom, ClientId, Doc, DocAttr};
use cdoc_model::{Alignment, Block, Comment, CommentId, Document, Inline, Paragraph};
use cdoc_model::{ParagraphProperties, RunProperties};
use proptest::prelude::*;

const PARAGRAPHS: usize = 3;
const FIELDS: usize = 5;
const COMMENTS: usize = 5;
const ALIGN: [Option<Alignment>; 3] = [None, Some(Alignment::Center), Some(Alignment::Justify)];

/// Sets field `f` of `p` from the choice `v`.
fn set_field(p: &mut ParagraphProperties, f: usize, v: u8) {
    let flag = [None, Some(false), Some(true)][usize::from(v % 3)];
    match f {
        0 => p.alignment = ALIGN[usize::from(v % 3)],
        1 => p.keep_with_next = flag,
        2 => p.keep_lines = flag,
        3 => p.page_break_before = flag,
        _ => p.outline_level = (!v.is_multiple_of(3)).then_some(v % 9),
    }
}

fn comment(k: usize, text: u8, resolved: bool) -> Comment {
    Comment {
        id: CommentId::new(format!("c{k}")),
        author: "A".to_owned(),
        date: "2026-10-17T00:00:00Z".to_owned(),
        text: format!("t{text}"),
        resolved,
        parent: None,
    }
}

fn base() -> Doc {
    let mut d = Document::blank();
    d.sections[0].blocks = (0..PARAGRAPHS)
        .map(|_| {
            Block::Paragraph(Paragraph {
                properties: ParagraphProperties::none(),
                inlines: vec![Inline::text("x", RunProperties::none())],
            })
        })
        .collect();
    d.comments = (0..3).map(|k| comment(k, 0, false)).collect();
    from_model(&d, ClientId(100)).unwrap()
}

fn paragraphs(d: &Doc) -> Vec<ParagraphProperties> {
    let m = to_model(d).unwrap();
    m.sections[0]
        .blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => p.properties.clone(),
            Block::Table(_) => unreachable!("only paragraphs"),
        })
        .collect()
}

#[derive(Debug, Clone)]
struct Case {
    n: usize,
    field_owner: Vec<Option<usize>>,
    comment_owner: Vec<Option<usize>>,
    field_edits: Vec<(usize, u8)>,
    comment_edits: Vec<(usize, u8)>,
    rotation: usize,
}

fn case() -> BoxedStrategy<Case> {
    (2usize..=5)
        .prop_flat_map(|n| {
            (
                prop::collection::vec(prop::option::of(0..n), PARAGRAPHS * FIELDS),
                prop::collection::vec(prop::option::of(0..n), COMMENTS),
                prop::collection::vec((0..PARAGRAPHS * FIELDS, any::<u8>()), 0..24),
                prop::collection::vec((0..COMMENTS, any::<u8>()), 0..12),
                any::<usize>(),
            )
                .prop_map(move |(fo, co, fe, ce, rotation)| Case {
                    n,
                    field_owner: fo,
                    comment_owner: co,
                    field_edits: fe,
                    comment_edits: ce,
                    rotation,
                })
        })
        .boxed()
}

/// The owner's edit of one comment: add it, rewrite it, toggle resolved, or remove it.
fn edit_comment(list: &mut Vec<Comment>, k: usize, v: u8) {
    let id = format!("c{k}");
    let at = list.iter().position(|c| c.id.as_str() == id);
    match (v % 4, at) {
        (0, Some(at)) => {
            list.remove(at);
        }
        (1, Some(at)) => list[at].resolved = !list[at].resolved,
        (_, Some(at)) => list[at].text = format!("t{v}"),
        (_, None) => list.push(comment(k, v, false)),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn concurrent_edits_of_different_fields_and_comments_all_survive(c in case()) {
        let base = base();
        let snapshot = base.encode_snapshot();
        let mut docs: Vec<Doc> = (0..c.n)
            .map(|i| Doc::decode_snapshot(&snapshot, ClientId(i as u64 + 1)).unwrap())
            .collect();
        let mut messages = Vec::new();
        for &(slot, v) in &c.field_edits {
            let Some(owner) = c.field_owner[slot] else { continue };
            let (k, f) = (slot / FIELDS, slot % FIELDS);
            let d = &mut docs[owner];
            let before = d.state_vector();
            let mut p = paragraphs(d)[k].clone();
            set_field(&mut p, f, v);
            d.set_block(1 + 2 * k, &Atom::Paragraph(p.clone())).unwrap();
            prop_assert_eq!(&paragraphs(d)[k], &p, "an edit reads back as set");
            messages.push(d.encode_diff(&before));
        }
        for &(k, v) in &c.comment_edits {
            let Some(owner) = c.comment_owner[k] else { continue };
            let d = &mut docs[owner];
            let before = d.state_vector();
            let mut list = to_model(d).unwrap().comments;
            edit_comment(&mut list, k, v);
            d.set_attr(&DocAttr::Comments(list.clone())).unwrap();
            prop_assert_eq!(&to_model(d).unwrap().comments, &list);
            messages.push(d.encode_diff(&before));
        }
        // What each owner ended with, before anything is exchanged.
        let local: Vec<_> = docs.iter().map(|d| (paragraphs(d), to_model(d).unwrap().comments)).collect();
        let base_paragraphs = paragraphs(&base);
        let base_comments = to_model(&base).unwrap().comments;
        docs.push(Doc::decode_snapshot(&snapshot, ClientId(50)).unwrap());
        for (i, d) in docs.iter_mut().enumerate() {
            let k = messages.len();
            for j in 0..k {
                d.apply_update(&messages[(k - 1 - j + i + c.rotation % k.max(1)) % k]).unwrap();
            }
            prop_assert!(!d.has_pending());
        }
        let model = to_model(&docs[0]).unwrap();
        for d in &docs {
            prop_assert_eq!(&to_model(d).unwrap(), &model);
            prop_assert_eq!(d.encode_snapshot(), docs[0].encode_snapshot());
        }
        let merged = paragraphs(&docs[0]);
        for slot in 0..PARAGRAPHS * FIELDS {
            let (k, f) = (slot / FIELDS, slot % FIELDS);
            let source = c.field_owner[slot].map_or(&base_paragraphs, |o| &local[o].0);
            let field = |p: &ParagraphProperties| format!("{:?}", [
                format!("{:?}", p.alignment), format!("{:?}", p.keep_with_next),
                format!("{:?}", p.keep_lines), format!("{:?}", p.page_break_before),
                format!("{:?}", p.outline_level),
            ][f]);
            prop_assert_eq!(field(&merged[k]), field(&source[k]), "paragraph {} field {}", k, f);
        }
        for k in 0..COMMENTS {
            let id = format!("c{k}");
            let source = c.comment_owner[k].map_or(&base_comments, |o| &local[o].1);
            let find = |l: &[Comment]| l.iter().find(|x| x.id.as_str() == id).cloned();
            prop_assert_eq!(find(&model.comments), find(source), "comment {}", id);
        }
    }
}

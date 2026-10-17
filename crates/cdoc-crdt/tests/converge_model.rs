//! Convergence of the whole document: 2 to 5 replicas edit text, blocks, formatting, block
//! properties and attributes concurrently; updates are delivered in arbitrary orders, with
//! duplicates and out of causal order. Every replica must end with the same model and the same
//! snapshot bytes.

use cdoc_crdt::{from_model, to_model, Atom, ClientId, Doc, DocAttr, Mark, RunKey};
use cdoc_model::{
    Alignment, BreakKind, CellProperties, CommentId, DocumentMeta, InlineContent, Length, Link,
    ParagraphProperties, RunProperties, TableProperties, TableWidth,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Edit {
    Text(String),
    Delete(u8),
    Paragraph(Option<Alignment>),
    Inline(bool),
    Table,
    Format(u8, u8),
    SetBlock(Option<Alignment>),
    Title(String),
}

#[derive(Debug, Clone)]
enum Step {
    Edit { replica: usize, at: u16, edit: Edit },
    Deliver { message: u16, to: usize },
    Sync { from: usize, to: usize },
}

const ALIGN: [Option<Alignment>; 3] = [None, Some(Alignment::Center), Some(Alignment::Right)];

fn edit() -> BoxedStrategy<Edit> {
    prop_oneof![
        4 => "[a-c]{1,3}".prop_map(Edit::Text),
        2 => (1u8..5).prop_map(Edit::Delete),
        1 => prop::sample::select(&ALIGN[..]).prop_map(Edit::Paragraph),
        1 => any::<bool>().prop_map(Edit::Inline),
        1 => Just(Edit::Table),
        3 => (0u8..10, 1u8..6).prop_map(|(m, len)| Edit::Format(m, len)),
        1 => prop::sample::select(&ALIGN[..]).prop_map(Edit::SetBlock),
        1 => "[x-z]{1,2}".prop_map(Edit::Title),
    ]
    .boxed()
}

fn scenario() -> BoxedStrategy<(usize, Vec<Step>)> {
    (2usize..=5)
        .prop_flat_map(|n| {
            let step = prop_oneof![
                5 => (0..n, any::<u16>(), edit())
                    .prop_map(|(replica, at, edit)| Step::Edit { replica, at, edit }),
                3 => (any::<u16>(), 0..n).prop_map(|(message, to)| Step::Deliver { message, to }),
                1 => (0..n, 0..n).prop_map(|(from, to)| Step::Sync { from, to }),
            ];
            (Just(n), prop::collection::vec(step, 1..50))
        })
        .boxed()
}

fn mark(m: u8) -> Mark {
    let bold = |on| RunProperties {
        bold: Some(on),
        ..RunProperties::none()
    };
    let comment = |id: &str, anchored| Mark::Comment {
        id: CommentId::new(id),
        anchored,
    };
    match m {
        0 => Mark::Run(bold(true)),
        1 => Mark::Run(bold(false)),
        2 => Mark::ClearRun(RunKey::Bold),
        3 => Mark::Run(RunProperties {
            italic: Some(true),
            ..RunProperties::none()
        }),
        4 => Mark::Link(Some(Link {
            href: "https://a.test".to_owned(),
            title: None,
        })),
        5 => Mark::Link(None),
        6 => comment("c1", true),
        7 => comment("c2", true),
        8 => comment("c1", false),
        _ => Mark::ClearRun(RunKey::Italic),
    }
}

fn paragraph(alignment: Option<Alignment>) -> Atom {
    Atom::Paragraph(ParagraphProperties {
        alignment,
        ..ParagraphProperties::none()
    })
}

/// Applies an edit at a position scaled into `1..=len` (unit 0 is the first section).
fn apply(d: &mut Doc, at: u16, edit: &Edit) {
    let len = d.len();
    let pos = 1 + usize::from(at) * len / 65_536;
    match edit {
        Edit::Text(t) => d.insert_text(pos, t).unwrap(),
        Edit::Delete(n) if pos < len => d.delete(pos, usize::from(*n).min(len - pos)).unwrap(),
        Edit::Paragraph(a) => d.insert_atom(pos, &paragraph(*a)).unwrap(),
        Edit::Inline(tab) => {
            let c = if *tab {
                InlineContent::Tab
            } else {
                InlineContent::Break {
                    kind: BreakKind::Line,
                }
            };
            d.insert_atom(pos, &Atom::Inline(c)).unwrap();
        }
        Edit::Table => {
            let atoms = [
                Atom::Table {
                    properties: TableProperties {
                        style: None,
                        width: TableWidth::Auto,
                        alignment: None,
                        borders: None,
                        cell_padding: None,
                    },
                    column_widths: vec![Length(1000)],
                },
                Atom::Row {
                    height: None,
                    cant_split: false,
                },
                Atom::Cell {
                    properties: CellProperties::single(),
                    header: false,
                },
                paragraph(None),
                Atom::End,
            ];
            for (i, a) in atoms.iter().enumerate() {
                d.insert_atom(pos + i, a).unwrap();
            }
        }
        Edit::Format(m, n) if pos < len => {
            let end = (pos + usize::from(*n)).min(len);
            d.format(pos..end, &mark(*m)).unwrap();
        }
        Edit::SetBlock(a) if pos < len => {
            // Only lands when the unit is a paragraph start; anything else is refused.
            let _refused_unless_paragraph = d.set_block(pos, &paragraph(*a));
        }
        Edit::Title(t) => d
            .set_attr(&DocAttr::Meta(DocumentMeta {
                title: Some(t.clone()),
                language: None,
            }))
            .unwrap(),
        Edit::Delete(_) | Edit::Format(..) | Edit::SetBlock(_) => {}
    }
}

fn run(n: usize, steps: &[Step]) -> Vec<Doc> {
    let base = from_model(&cdoc_model::Document::blank(), ClientId(100)).unwrap();
    let snapshot = base.encode_snapshot();
    let mut docs: Vec<Doc> = (0..n)
        .map(|i| Doc::decode_snapshot(&snapshot, ClientId(i as u64 + 1)).unwrap())
        .collect();
    let mut messages: Vec<Vec<u8>> = Vec::new();
    for s in steps {
        match s {
            Step::Edit { replica, at, edit } => {
                let before = docs[*replica].state_vector();
                apply(&mut docs[*replica], *at, edit);
                messages.push(docs[*replica].encode_diff(&before));
            }
            Step::Deliver { message, to } if !messages.is_empty() => {
                let m = &messages[usize::from(*message) % messages.len()];
                docs[*to].apply_update(m).unwrap();
            }
            Step::Deliver { .. } => {}
            Step::Sync { from, to } => {
                let update = docs[*from].encode_diff(&docs[*to].state_vector());
                docs[*to].apply_update(&update).unwrap();
            }
        }
    }
    for (i, d) in docs.iter_mut().enumerate() {
        // Each replica gets every message, newest first and rotated, so most arrive before
        // what they depend on.
        let k = messages.len();
        for j in 0..k {
            d.apply_update(&messages[(k - 1 - j + i * 3) % k]).unwrap();
        }
    }
    docs
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn replicas_converge_on_the_model_and_the_bytes((n, steps) in scenario()) {
        let docs = run(n, &steps);
        let model = to_model(&docs[0]).unwrap();
        let snapshot = docs[0].encode_snapshot();
        for d in &docs {
            prop_assert!(!d.has_pending());
            prop_assert_eq!(&to_model(d).unwrap(), &model);
            prop_assert_eq!(&d.encode_snapshot(), &snapshot);
        }
        let reloaded = Doc::decode_snapshot(&snapshot, ClientId(50)).unwrap();
        prop_assert_eq!(to_model(&reloaded).unwrap(), model);
    }
}

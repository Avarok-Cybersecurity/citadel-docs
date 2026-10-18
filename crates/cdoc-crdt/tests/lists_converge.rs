//! List-valued attributes (comments, styles, numbering definitions and instances) are maps by
//! id: replicas that concurrently add and remove elements, comments anchored in the text, all
//! converge on the same document with every element kept that was not removed, and that
//! document validates (no mark names a lost comment).

use cdoc_crdt::{from_model, to_model, ClientId, Doc, DocAttr, Mark};
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Comment, CommentId, Document, Level, NumId,
    NumberFormat, NumberingInstance, StyleId,
};
use proptest::prelude::*;
use std::collections::BTreeSet;

const REPLICAS: usize = 3;
const LISTS: u8 = 3;

#[derive(Debug, Clone)]
enum Step {
    /// Replica, list (0 comment, 1 style, 2 numbering definition and instance).
    Add(usize, u8),
    /// Replica, list, which of its own additions to remove.
    Remove(usize, u8, usize),
    /// From, to.
    Deliver(usize, usize),
}

fn step() -> BoxedStrategy<Step> {
    prop_oneof![
        3 => (0..REPLICAS, 0..LISTS).prop_map(|(r, l)| Step::Add(r, l)),
        1 => (0..REPLICAS, 0..LISTS, any::<usize>()).prop_map(|(r, l, k)| Step::Remove(r, l, k)),
        2 => (0..REPLICAS, 0..REPLICAS).prop_map(|(a, b)| Step::Deliver(a, b)),
    ]
    .boxed()
}

fn base() -> Doc {
    let mut d = from_model(&Document::blank(), ClientId(100)).unwrap();
    d.insert_text(2, "anchors").unwrap();
    d
}

/// The element of `list` named `n` (unique across replicas), as the model holds it.
fn add(d: &mut Doc, list: u8, n: u32) {
    let mut m = to_model(d).unwrap();
    match list {
        0 => {
            let id = CommentId::new(format!("c{n}"));
            m.comments.push(Comment {
                id: id.clone(),
                author: format!("a{n}"),
                date: String::new(),
                text: format!("t{n}"),
                resolved: false,
                parent: None,
            });
            d.set_attr(&DocAttr::Comments(m.comments)).unwrap();
            let at = 2 + (n as usize % 6);
            d.format(at..at + 1, &Mark::Comment { id, anchored: true })
                .unwrap();
        }
        1 => {
            let mut style = m.styles.styles[0].clone();
            style.id = StyleId::new(format!("s{n}"));
            style.name = format!("S{n}");
            m.styles.styles.push(style);
            d.set_attr(&DocAttr::Styles(m.styles)).unwrap();
        }
        _ => {
            m.numbering.definitions.push(AbstractNumbering {
                id: AbstractNumId(n),
                levels: vec![Level {
                    start: n,
                    format: NumberFormat::Decimal,
                    text: "%1.".to_owned(),
                    alignment: Alignment::Left,
                    indent: None,
                    run: None,
                }],
            });
            m.numbering.instances.push(NumberingInstance {
                id: NumId(n),
                abstract_id: AbstractNumId(n),
                overrides: Vec::new(),
            });
            d.set_attr(&DocAttr::Numbering(m.numbering)).unwrap();
        }
    }
}

fn remove(d: &mut Doc, list: u8, n: u32) {
    let mut m = to_model(d).unwrap();
    match list {
        0 => {
            let id = CommentId::new(format!("c{n}"));
            let at = 2 + (n as usize % 6);
            d.format(
                at..at + 1,
                &Mark::Comment {
                    id: id.clone(),
                    anchored: false,
                },
            )
            .unwrap();
            m.comments.retain(|c| c.id != id);
            d.set_attr(&DocAttr::Comments(m.comments)).unwrap();
        }
        1 => {
            m.styles.styles.retain(|s| s.id.as_str() != format!("s{n}"));
            d.set_attr(&DocAttr::Styles(m.styles)).unwrap();
        }
        _ => {
            m.numbering.definitions.retain(|x| x.id != AbstractNumId(n));
            m.numbering.instances.retain(|x| x.id != NumId(n));
            d.set_attr(&DocAttr::Numbering(m.numbering)).unwrap();
        }
    }
}

fn deliver(from: &Doc, to: &mut Doc) {
    to.apply_update(&from.encode_diff(&to.state_vector()))
        .unwrap();
}

/// The ids of the added elements a model holds, per list, as names `n`.
fn held(m: &Document) -> [BTreeSet<u32>; 3] {
    let num = |s: &str, p: char| s.strip_prefix(p).and_then(|n| n.parse().ok());
    [
        m.comments
            .iter()
            .filter_map(|c| num(c.id.as_str(), 'c'))
            .collect(),
        m.styles
            .styles
            .iter()
            .filter_map(|s| num(s.id.as_str(), 's'))
            .collect(),
        m.numbering.definitions.iter().map(|d| d.id.0).collect(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]

    #[test]
    fn concurrent_list_edits_converge_and_lose_nothing(
        steps in prop::collection::vec(step(), 1..40),
    ) {
        let snapshot = base().encode_snapshot();
        let mut docs: Vec<Doc> = (0..REPLICAS)
            .map(|i| Doc::decode_snapshot(&snapshot, ClientId(i as u64 + 1)).unwrap())
            .collect();
        let mut own: Vec<Vec<Vec<u32>>> = vec![vec![Vec::new(); 3]; REPLICAS];
        let mut expected: [BTreeSet<u32>; 3] = Default::default();
        for (i, s) in steps.iter().enumerate() {
            match *s {
                Step::Add(r, l) => {
                    // Numbering ids start at 1000: the blank document's are below.
                    let n = 1000 + u32::try_from(i).unwrap();
                    add(&mut docs[r], l, n);
                    own[r][usize::from(l)].push(n);
                    expected[usize::from(l)].insert(n);
                }
                Step::Remove(r, l, k) => {
                    let mine = &mut own[r][usize::from(l)];
                    if mine.is_empty() {
                        continue;
                    }
                    let n = mine.remove(k % mine.len());
                    remove(&mut docs[r], l, n);
                    expected[usize::from(l)].remove(&n);
                }
                Step::Deliver(a, b) if a != b => {
                    let from = docs[a].clone();
                    deliver(&from, &mut docs[b]);
                }
                Step::Deliver(..) => {}
            }
        }
        for _ in 0..2 {
            for a in 0..REPLICAS {
                for b in 0..REPLICAS {
                    if a != b {
                        let from = docs[a].clone();
                        deliver(&from, &mut docs[b]);
                    }
                }
            }
        }
        let first = to_model(&docs[0]).unwrap();
        for d in &docs[1..] {
            prop_assert_eq!(d.encode_snapshot(), docs[0].encode_snapshot());
            prop_assert_eq!(&to_model(d).unwrap(), &first);
        }
        prop_assert_eq!(held(&first), expected);
        prop_assert!(first.validate().is_ok(), "{:?}", first.validate());
    }
}

//! Convergence across garbage collection: replicas edit and sync through a relay and with each
//! other; the relay collects with every replica's state vector as the known peers (twice), a
//! late replica joins from the relay's collected snapshot, and every replica (relay included)
//! must end with the same document. Every update between peers at or beyond the floor applies.

mod common;

use cdoc_crdt::{from_model, to_model, Atom, ClientId, Doc, Mark};
use cdoc_model::{ParagraphProperties, RunProperties};
use common::ROOM;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Edit {
    Text(String),
    Delete(u8),
    Paragraph,
    Bold(u8),
}

#[derive(Debug, Clone)]
enum Step {
    Edit { replica: usize, at: u16, edit: Edit },
    Up(usize),
    Down(usize),
    Direct { from: usize, to: usize },
}

fn steps(n: usize) -> BoxedStrategy<Vec<Step>> {
    let edit = prop_oneof![
        4 => "[a-d]{1,4}".prop_map(Edit::Text),
        3 => (1u8..6).prop_map(Edit::Delete),
        1 => Just(Edit::Paragraph),
        1 => (1u8..5).prop_map(Edit::Bold),
    ];
    let step = prop_oneof![
        5 => (0..n, any::<u16>(), edit).prop_map(|(replica, at, edit)| Step::Edit { replica, at, edit }),
        2 => (0..n).prop_map(Step::Up),
        2 => (0..n).prop_map(Step::Down),
        1 => (0..n, 0..n).prop_map(|(from, to)| Step::Direct { from, to }),
    ];
    prop::collection::vec(step, 1..40).boxed()
}

fn scenario() -> BoxedStrategy<(usize, Vec<Step>, Vec<Step>)> {
    (2usize..=4)
        .prop_flat_map(|n| (Just(n), steps(n), steps(n + 1)))
        .boxed()
}

fn apply(d: &mut Doc, at: u16, edit: &Edit) {
    let len = d.len();
    let pos = 1 + usize::from(at) * len / 65_536;
    match edit {
        Edit::Text(t) => d.insert_text(pos, t).unwrap(),
        Edit::Delete(k) if pos < len => d.delete(pos, usize::from(*k).min(len - pos)).unwrap(),
        Edit::Paragraph => d
            .insert_atom(pos, &Atom::Paragraph(ParagraphProperties::none()))
            .unwrap(),
        Edit::Bold(k) if pos < len => {
            let bold = RunProperties {
                bold: Some(true),
                ..RunProperties::none()
            };
            let end = (pos + usize::from(*k)).min(len);
            d.format(pos..end, &Mark::Run(bold)).unwrap();
        }
        Edit::Delete(_) | Edit::Bold(_) => {}
    }
}

fn sync(from: &Doc, to: &mut Doc) {
    to.apply_update(&from.encode_diff(&to.state_vector()), ROOM)
        .unwrap();
}

fn run(docs: &mut [Doc], relay: &mut Doc, steps: &[Step]) {
    for s in steps {
        match *s {
            Step::Edit {
                replica,
                at,
                ref edit,
            } => apply(&mut docs[replica], at, edit),
            Step::Up(r) => sync(&docs[r], relay),
            Step::Down(r) => sync(relay, &mut docs[r]),
            Step::Direct { from, to } if from != to => {
                let update = docs[from].encode_diff(&docs[to].state_vector());
                docs[to].apply_update(&update, ROOM).unwrap();
            }
            Step::Direct { .. } => {}
        }
    }
}

/// Every replica reports its state vector to the relay, which then collects.
fn collect(docs: &[Doc], relay: &mut Doc) {
    for d in docs {
        sync(d, relay);
    }
    let model = to_model(relay).unwrap();
    let known: Vec<_> = docs.iter().map(Doc::state_vector).collect();
    relay.gc(&known).unwrap();
    assert_eq!(to_model(relay).unwrap(), model);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]

    #[test]
    fn peers_at_the_floor_converge_with_the_collected_relay((n, first, second) in scenario()) {
        let base = from_model(&cdoc_model::Document::blank(), ClientId(100)).unwrap();
        let snapshot = base.encode_snapshot();
        let mut relay = Doc::decode_snapshot(&snapshot, ClientId(99)).unwrap();
        let mut docs: Vec<Doc> = (0..n)
            .map(|i| Doc::decode_snapshot(&snapshot, ClientId(i as u64 + 1)).unwrap())
            .collect();
        run(&mut docs, &mut relay, &first);
        collect(&docs, &mut relay);
        docs.push(Doc::decode_snapshot(&relay.encode_snapshot(), ClientId(50)).unwrap());
        let (early, late) = second.split_at(second.len() / 2);
        run(&mut docs, &mut relay, early);
        collect(&docs, &mut relay);
        run(&mut docs, &mut relay, late);
        for _ in 0..2 {
            for d in &docs {
                sync(d, &mut relay);
            }
            for d in &mut docs {
                sync(&relay, d);
            }
        }
        let model = to_model(&relay).unwrap();
        for d in &docs {
            prop_assert!(!d.has_pending());
            prop_assert_eq!(&to_model(d).unwrap(), &model);
            prop_assert_eq!(d.visible_text(), relay.visible_text());
        }
        let reloaded = Doc::decode_snapshot(&relay.encode_snapshot(), ClientId(51)).unwrap();
        prop_assert_eq!(to_model(&reloaded).unwrap(), model);
    }
}

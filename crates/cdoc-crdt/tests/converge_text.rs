//! Convergence of the sequence alone: replicas editing text concurrently, updates delivered in
//! any order with duplicates, end in identical states.

use cdoc_crdt::{ClientId, Doc};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    Insert {
        replica: usize,
        at: u16,
        text: String,
    },
    Delete {
        replica: usize,
        at: u16,
        len: u8,
    },
    Deliver {
        message: u16,
        to: usize,
    },
}

fn step(replicas: usize) -> BoxedStrategy<Step> {
    prop_oneof![
        4 => (0..replicas, any::<u16>(), "[a-c]{1,4}")
            .prop_map(|(replica, at, text)| Step::Insert { replica, at, text }),
        2 => (0..replicas, any::<u16>(), 1u8..4)
            .prop_map(|(replica, at, len)| Step::Delete { replica, at, len }),
        3 => (any::<u16>(), 0..replicas).prop_map(|(message, to)| Step::Deliver { message, to }),
    ]
    .boxed()
}

fn scenario(steps: std::ops::Range<usize>) -> BoxedStrategy<(usize, Vec<Step>, Vec<u16>)> {
    (2usize..=5)
        .prop_flat_map(move |n| {
            (
                Just(n),
                prop::collection::vec(step(n), steps.clone()),
                prop::collection::vec(any::<u16>(), 0..40),
            )
        })
        .boxed()
}

fn scale(at: u16, len: usize) -> usize {
    usize::from(at) * (len + 1) / 65_536
}

/// Runs the steps; every edit is broadcast as its own update. Finally every replica receives
/// every update, in an order shuffled by `order`.
fn run(n: usize, steps: &[Step], order: &[u16]) -> Vec<Doc> {
    let mut docs: Vec<Doc> = (0..n).map(|i| Doc::new(ClientId(i as u64 + 1))).collect();
    let mut messages: Vec<Vec<u8>> = Vec::new();
    for s in steps {
        match s {
            Step::Insert { replica, at, text } => {
                let d = &mut docs[*replica];
                let before = d.state_vector();
                d.insert_text(scale(*at, d.len()), text).unwrap();
                messages.push(d.encode_diff(&before));
            }
            Step::Delete { replica, at, len } => {
                let d = &mut docs[*replica];
                if d.is_empty() {
                    continue;
                }
                let start = scale(*at, d.len() - 1);
                let len = usize::from(*len).min(d.len() - start);
                let before = d.state_vector();
                d.delete(start, len).unwrap();
                messages.push(d.encode_diff(&before));
            }
            Step::Deliver { message, to } if !messages.is_empty() => {
                let m = &messages[usize::from(*message) % messages.len()];
                docs[*to].apply_update(m).unwrap();
            }
            Step::Deliver { .. } => {}
        }
    }
    let mut all: Vec<usize> = (0..messages.len()).collect();
    for (i, &o) in order.iter().enumerate() {
        if all.len() > 1 {
            let j = usize::from(o) % all.len();
            let k = i % all.len();
            all.swap(k, j);
        }
    }
    for d in &mut docs {
        for &m in &all {
            d.apply_update(&messages[m]).unwrap();
        }
    }
    docs
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn replicas_converge((n, steps, order) in scenario(1..60)) {
        check(n, &steps, &order)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    /// Long histories: enough items to split the sequence index's leaves.
    #[test]
    fn replicas_converge_over_long_histories((n, steps, order) in scenario(600..900)) {
        check(n, &steps, &order)?;
    }
}

fn check(n: usize, steps: &[Step], order: &[u16]) -> Result<(), TestCaseError> {
    let docs = run(n, steps, order);
    let snapshot = docs[0].encode_snapshot();
    for d in &docs {
        prop_assert!(!d.has_pending());
        prop_assert_eq!(d.visible_text(), docs[0].visible_text());
        prop_assert_eq!(d.state_vector(), docs[0].state_vector());
        prop_assert_eq!(&d.encode_snapshot(), &snapshot);
    }
    let reloaded = Doc::decode_snapshot(&snapshot, ClientId(99)).unwrap();
    prop_assert_eq!(reloaded.visible_text(), docs[0].visible_text());
    prop_assert_eq!(reloaded.encode_snapshot(), snapshot);
    Ok(())
}

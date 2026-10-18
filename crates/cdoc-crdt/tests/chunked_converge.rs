//! Chunked diffs between replicas that edited concurrently (text, deletes, marks, block
//! attributes, partial syncs), under random limits: every chunk is within the limit, applied in
//! order none waits, the result is the unchunked diff's, and applied in reverse the buffered
//! chunks still converge to it. A limit too small for one operation is refused, never exceeded.

mod common;

use cdoc_crdt::{from_model, ChunkError, ClientId, Doc, Id, Mark, RunKey};
use cdoc_model::{Document, RunProperties};
use common::ROOM;
use proptest::prelude::*;

const REPLICAS: usize = 3;

#[derive(Debug, Clone)]
enum Step {
    Insert(usize, usize, usize),
    Delete(usize, usize, usize),
    Bold(usize, usize, usize, bool),
    Deliver(usize, usize),
}

fn step() -> BoxedStrategy<Step> {
    prop_oneof![
        4 => (0..REPLICAS, any::<usize>(), 1..300_usize).prop_map(|(r, p, n)| Step::Insert(r, p, n)),
        2 => (0..REPLICAS, any::<usize>(), 1..40_usize).prop_map(|(r, p, n)| Step::Delete(r, p, n)),
        1 => (0..REPLICAS, any::<usize>(), 1..40_usize, any::<bool>())
            .prop_map(|(r, p, n, on)| Step::Bold(r, p, n, on)),
        2 => (0..REPLICAS, 0..REPLICAS).prop_map(|(a, b)| Step::Deliver(a, b)),
    ]
    .boxed()
}

const ALPHABET: &str = "abcdefghij klmnopé😀qrstuvwxyz";

fn text(n: usize) -> String {
    ALPHABET.chars().cycle().take(n).collect()
}

/// The blank document is a section atom and a paragraph atom; its text follows them.
const TEXT_START: usize = 2;

/// Up to `n` characters of the paragraph's text from a position `p` picks, if it has any.
fn span(d: &Doc, p: usize, n: usize) -> Option<(usize, usize)> {
    let chars = d.len().checked_sub(TEXT_START).filter(|&c| c > 0)?;
    let at = p % chars;
    Some((TEXT_START + at, n.min(chars - at)))
}

fn run(steps: &[Step]) -> Vec<Doc> {
    let base = from_model(&Document::blank(), ClientId(100)).unwrap();
    let mut ds: Vec<Doc> = (0..REPLICAS)
        .map(|r| {
            let mut d = Doc::new(ClientId(r as u64 + 1));
            d.apply_update(&base.encode_diff(&d.state_vector()), ROOM)
                .unwrap();
            d
        })
        .collect();
    for s in steps {
        match *s {
            Step::Insert(r, p, n) => {
                let at = TEXT_START + p % (ds[r].len() - TEXT_START + 1);
                ds[r].insert_text(at, &text(n)).unwrap();
            }
            Step::Delete(r, p, n) => {
                if let Some((at, len)) = span(&ds[r], p, n) {
                    ds[r].delete(at, len).unwrap();
                }
            }
            Step::Bold(r, p, n, on) => {
                let mark = if on {
                    Mark::Run(RunProperties {
                        bold: Some(true),
                        ..RunProperties::none()
                    })
                } else {
                    Mark::ClearRun(RunKey::Bold)
                };
                if let Some((at, len)) = span(&ds[r], p, n) {
                    ds[r].format(at..at + len, &mark).unwrap();
                }
            }
            Step::Deliver(a, b) if a != b => {
                let update = ds[a].encode_diff(&ds[b].state_vector());
                ds[b].apply_update(&update, ROOM).unwrap();
            }
            Step::Deliver(..) => {}
        }
    }
    ds
}

fn check(sender: &Doc, receiver: &Doc, limit: usize) -> Result<(), TestCaseError> {
    let sv = receiver.state_vector();
    let chunks = match sender.encode_diff_chunked(&sv, limit) {
        Ok(chunks) => chunks,
        Err(ChunkError::OversizeOperation {
            bytes, max_bytes, ..
        }) => {
            prop_assert!(bytes > max_bytes);
            return Ok(());
        }
        Err(e) => return Err(TestCaseError::fail(format!("{e}"))),
    };
    let mut whole = receiver.clone();
    whole.apply_update(&sender.encode_diff(&sv), ROOM).unwrap();
    let mut in_order = receiver.clone();
    for chunk in &chunks {
        prop_assert!(chunk.len() <= limit, "{} > {limit}", chunk.len());
        prop_assert_eq!(
            in_order.apply_update(chunk, ROOM).unwrap().missing,
            Vec::<Id>::new()
        );
    }
    prop_assert_eq!(in_order.encode_snapshot(), whole.encode_snapshot());
    let mut reversed = receiver.clone();
    for chunk in chunks.iter().rev() {
        reversed.apply_update(chunk, ROOM).unwrap();
    }
    prop_assert!(!reversed.has_pending());
    prop_assert_eq!(reversed.encode_snapshot(), whole.encode_snapshot());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn chunked_diffs_converge_like_the_unchunked_diff(
        steps in proptest::collection::vec(step(), 1..40),
        limit in 8..2_048_usize,
    ) {
        let ds = run(&steps);
        for sender in &ds {
            for receiver in &ds {
                check(sender, receiver, limit)?;
            }
            check(sender, &Doc::new(ClientId(50)), limit)?;
        }
    }
}

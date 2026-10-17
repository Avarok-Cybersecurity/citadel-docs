//! The sequence against a plain `Vec<char>`: thousands of edits, enough to split the index's
//! leaves many times, local and remote, must leave the same text.

use cdoc_crdt::{ClientId, Doc, StateVector};
use proptest::prelude::*;

fn ops() -> BoxedStrategy<Vec<(bool, u16, u8)>> {
    prop::collection::vec(
        (prop::bool::weighted(0.7), any::<u16>(), 1u8..6),
        1500..3000,
    )
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn local_edits_match_a_vector(ops in ops()) {
        let mut doc = Doc::new(ClientId(1));
        let mut naive: Vec<char> = Vec::new();
        let mut remote = Doc::new(ClientId(2));
        for (i, &(insert, at, n)) in ops.iter().enumerate() {
            let before = doc.state_vector();
            if insert || naive.is_empty() {
                let pos = usize::from(at) * (naive.len() + 1) / 65_536;
                let shift = u8::try_from(i % 26).unwrap();
                let text: String = (0..n).map(|k| char::from(b'a' + (shift + k) % 26)).collect();
                doc.insert_text(pos, &text).unwrap();
                naive.splice(pos..pos, text.chars());
            } else {
                let pos = usize::from(at) * naive.len() / 65_536;
                let len = usize::from(n).min(naive.len() - pos);
                doc.delete(pos, len).unwrap();
                naive.drain(pos..pos + len);
            }
            remote.apply_update(&doc.encode_diff(&before)).unwrap();
        }
        let expected: String = naive.iter().collect();
        prop_assert_eq!(doc.len(), naive.len());
        prop_assert_eq!(&doc.visible_text(), &expected);
        prop_assert_eq!(&remote.visible_text(), &expected);
        prop_assert_eq!(remote.encode_snapshot(), doc.encode_snapshot());
        let mut fresh = Doc::new(ClientId(3));
        fresh.apply_update(&doc.encode_diff(&StateVector::empty())).unwrap();
        prop_assert_eq!(fresh.visible_text(), expected);
    }
}

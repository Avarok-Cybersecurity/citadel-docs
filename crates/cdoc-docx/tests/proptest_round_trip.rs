//! Property: for every generated document, reading what was written gives the document's
//! projection onto what `.docx` can hold.

mod common;

use cdoc_model::test_support::strategies::document;
use proptest::prelude::*;

proptest! {
    #[test]
    fn generated_documents_round_trip(doc in document()) {
        let expected = common::canon::canonical(doc);
        let (back, written, _read) = common::round_trip(&expected);
        prop_assert!(back == expected, "{}", common::first_diff(&back, &expected).unwrap_or_default());
        prop_assert!(written.is_lossless(), "write losses: {:?}", written);
    }
}

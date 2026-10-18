//! Property: for every generated document, reading what was written gives the document's
//! projection, the reader reports nothing, and the writer reports only values ODF has no slot
//! for (parked in the sidecar, at a bounded set of places) and a bounded set of constructs
//! written as their nearest ODF equivalent. A native mapping that regresses shows up as a new
//! parked place.

mod common;

use cdoc_model::test_support::strategies::document;
use proptest::prelude::*;

/// Where a generated value may have no ODF slot.
const PARKED: [&str; 34] = [
    "section/page/margins/",
    "settings/autoHyphenation",
    "text/kern",
    "text/ligatures",
    "text/lang",
    "paragraph/suppressAutoHyphens",
    "default/paragraph/suppressAutoHyphens",
    "default/run/kern",
    "default/run/ligatures",
    "default/run/lang",
    "section/start",
    "section/page/pageNumbering/format",
    "settings/evenAndOddHeaders",
    "paragraph/borders",
    "paragraph/indent",
    "paragraph/spacing",
    "default/paragraph/style",
    "default/paragraph/outlineLevel",
    "default/paragraph/numbering",
    "default/paragraph/borders",
    "default/paragraph/indent",
    "default/paragraph/spacing",
    "default/run/style",
    "style/paragraph/",
    "style/run/",
    "table/properties/borders",
    "table/properties/cellPadding",
    "table-cell/properties/width",
    "table-cell/properties/rowSpan",
    "table-cell/properties/colSpan",
    "table-cell/properties/borders",
    "table-cell/header",
    "list/id",
    "list/levels/*/",
];

/// Constructs written as their nearest ODF equivalent.
const NEAREST: [&str; 5] = [
    "a footnote nothing refers to written as a note in the first paragraph",
    "a first-page header or footer without a title page kept in an unused master page",
    "a deleted page or column break written as a break",
    "a footnote referenced more than once; later references written as note references",
    "a comment anchored on separate ranges written as one annotation per range",
];

fn expected_loss(what: &str) -> bool {
    match what.strip_suffix(&format!(" {}", common::NO_SLOT)) {
        Some(place) => PARKED.iter().any(|p| place.starts_with(p)),
        None => NEAREST.contains(&what),
    }
}

proptest! {
    #[test]
    fn generated_documents_round_trip(doc in document()) {
        let expected = common::canon::canonical(doc);
        let (back, written, read) = common::round_trip(&expected);
        prop_assert!(back == expected, "{}", common::first_diff(&back, &expected).unwrap_or_default());
        prop_assert!(read.losses.is_empty(), "read losses: {:?}", read.losses);
        let unexpected: Vec<&str> = written.losses.iter().filter_map(|l| l.what()).filter(|w| !expected_loss(w)).collect();
        prop_assert!(unexpected.is_empty(), "unexpected write losses: {unexpected:?}");
    }
}

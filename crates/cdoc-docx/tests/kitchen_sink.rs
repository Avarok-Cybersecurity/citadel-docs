//! Golden round trip: the kitchen-sink document survives model -> docx -> model unchanged.

mod common;

use cdoc_model::test_support::fixtures::kitchen_sink;

#[test]
fn kitchen_sink_round_trips_exactly() {
    let doc = kitchen_sink();
    let (back, written, read) = common::round_trip(&doc);
    assert_eq!(back, doc);
    assert!(written.is_lossless(), "write losses: {written:?}");
    let unexpected: Vec<_> = read.losses.iter().collect();
    assert!(unexpected.is_empty(), "read losses: {unexpected:?}");
}

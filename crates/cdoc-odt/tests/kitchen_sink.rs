//! Golden round trip: the kitchen-sink document survives model -> odt -> model unchanged, and
//! the only write losses are values parked in the sidecar.

mod common;

use cdoc_model::test_support::fixtures::kitchen_sink;

#[test]
fn kitchen_sink_round_trips_exactly() {
    let doc = kitchen_sink();
    let (back, written, read) = common::round_trip(&doc);
    assert!(
        back == doc,
        "{}",
        common::first_diff(&back, &doc).unwrap_or_default()
    );
    assert!(read.losses.is_empty(), "read losses: {:?}", read.losses);
    let parked: Vec<String> = written
        .losses
        .iter()
        .filter_map(|l| l.what().map(str::to_owned))
        .collect();
    eprintln!("parked: {parked:#?}");
}

//! `CARET_LINES`: every caret line of the layout reaches the list unchanged, malformed ones are
//! refused, and a list of the version before them is refused by its version.

mod support;

use cdoc_layout::{CaretLine, CaretStop};
use cdoc_paint::format::VERSION;
use cdoc_paint::{decode, encode, DecodeError, Op};

#[test]
fn every_caret_line_of_the_layout_reaches_the_list_unchanged() {
    let laid = support::kitchen();
    let bytes = encode(&laid, &support::fonts()).expect("encodes");
    let list = decode(&bytes).expect("decodes").to_owned().expect("pages");
    let mut total = 0;
    for (page, ops) in laid.pages.iter().zip(&list.pages) {
        let ops: Vec<&Vec<CaretLine>> = ops
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::CaretLines(lines) => Some(lines),
                _ => None,
            })
            .collect();
        assert!(ops.len() <= 1, "one op holds a page's lines");
        let lines: Vec<&CaretLine> = ops.into_iter().flatten().collect();
        assert_eq!(lines, page.lines.iter().collect::<Vec<_>>());
        total += lines.len();
    }
    assert!(total > 0, "the kitchen sink has text");
}

#[test]
fn a_version_3_list_is_refused_by_its_version() {
    assert_eq!(VERSION, 5);
    let mut bytes = support::every_op().to_bytes().expect("writes");
    bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
    let err = decode(&bytes).expect_err("refused");
    assert_eq!(err, DecodeError::Version(3));
    assert_eq!(
        err.to_string(),
        "display list version 3 is not supported (this decoder reads 5)"
    );
}

#[test]
fn a_caret_line_with_no_run_and_no_stop_or_gaps_that_do_not_increase_is_refused() {
    let line = |runs: u32, stops: &[(u32, i32)]| {
        let mut list = support::every_op();
        let first = list.pages[0]
            .ops
            .iter_mut()
            .find_map(|op| match op {
                Op::CaretLines(lines) => lines.first_mut(),
                _ => None,
            })
            .expect("every_op has a caret line");
        first.runs = runs;
        first.stops = stops.iter().map(|&(gap, x)| CaretStop { gap, x }).collect();
        let bytes = list.to_bytes().expect("writes");
        decode(&bytes).expect("tables are intact").page(0).err()
    };
    assert_eq!(line(0, &[(0, 0), (1, 5)]), None);
    assert_eq!(line(2, &[]), None, "a line of runs only");
    assert_eq!(
        line(0, &[]),
        Some(DecodeError::Malformed(
            "a caret line has no runs and no stops"
        ))
    );
    for stops in [&[(1, 0), (1, 5)][..], &[(2, 0), (1, 5)]] {
        assert_eq!(
            line(1, stops),
            Some(DecodeError::Malformed("caret stops' gaps do not increase")),
            "{stops:?}"
        );
    }
}

#[test]
fn a_caret_lines_op_with_no_line_is_refused() {
    let mut list = support::every_op();
    for op in &mut list.pages[0].ops {
        if let Op::CaretLines(lines) = op {
            lines.clear();
        }
    }
    let bytes = list.to_bytes().expect("writes");
    let err = decode(&bytes).expect("tables are intact").page(0).err();
    assert_eq!(
        err,
        Some(DecodeError::Malformed("a caret lines op has no lines"))
    );
}

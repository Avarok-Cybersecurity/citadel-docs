//! The documented mapping, pinned: the kitchen-sink document as text and what the writer
//! reports.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_txt::document_to_txt;

const EXPECTED: &str = "Kitchen sink\n\nEvery kind\n\n\
Body a link (https://avarok.net)\t[image: a picture]\n\
[1]4 October 2026no[signature: Sign here]\u{c}\n\n\
\u{20}\u{20}3. second level\n\n\
Merged header |\n\
--------------+-------\n\
a             | nested\n\n\
> Quoted\n\n\
Second section\n\n\
[1] A note\n";

#[test]
fn kitchen_sink_as_text() {
    let (text, _) = document_to_txt(&kitchen_sink());
    assert_eq!(text, EXPECTED);
}

#[test]
fn kitchen_sink_report_names_each_kind_of_loss() {
    let (_, report) = document_to_txt(&kitchen_sink());
    for (what, count) in [
        ("header", 6),
        ("footer", 2),
        ("character formatting", 2),
        ("comment anchor", 1),
        ("tracked insertion (accepted)", 1),
        ("tracked deletion (text removed)", 1),
        ("image (written as a placeholder)", 1),
        ("field (its cached text is written)", 1),
        ("form field (written as its value)", 1),
        ("signature field (written as a placeholder)", 1),
        ("merged cell (text in its first position)", 1),
        ("comment", 2),
    ] {
        assert_eq!(report.count_of(what), count, "{what}: {report:?}");
    }
}

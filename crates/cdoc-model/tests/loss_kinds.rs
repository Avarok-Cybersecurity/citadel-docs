//! Every loss has a severity and a label from one table, by what it is: what nothing visible
//! depends on is Info, formatting and structure Minor, content Major, and a loss the table does
//! not name Major, so it is never hidden.

use cdoc_model::image_probe::{held_opaque, ImageKind, ProbeError};
use cdoc_model::loss::{classify, labels, Kind, Severity};
use cdoc_model::Report;

fn kind(part: &str, what: &str) -> (Severity, &'static str) {
    let Kind { severity, label } = classify(part, what);
    (severity, label)
}

const DOC: &str = "word/document.xml";

#[test]
fn what_no_reader_sees_is_info() {
    for (part, what, label) in [
        ("word/settings.xml", "w:settings/w:zoom", labels::SETTINGS),
        (
            "docProps/core.xml",
            "cp:coreProperties/dc:creator",
            labels::PROPERTIES,
        ),
        (
            "meta.xml",
            "office:meta/meta:editing-cycles",
            labels::PROPERTIES,
        ),
        (DOC, "w:bookmarkStart", labels::EDITING_MARKS),
        ("word/styles.xml", "w:style/w:rsid", labels::EDITING_MARKS),
        (
            "content.xml",
            "style:text-properties@officeooo:rsid",
            labels::EDITING_MARKS,
        ),
        (DOC, "w:rPr/w:szCs", labels::SCRIPTS),
        (
            "styles.xml",
            "style:text-properties@style:font-name-complex",
            labels::SCRIPTS,
        ),
        ("docProps/app.xml", "part not read", labels::PROPERTIES),
        ("word/theme/theme1.xml", "part not read", labels::SETTINGS),
        (
            "(model)",
            "a style based on an undefined style; link dropped",
            labels::REPAIRED,
        ),
        (
            DOC,
            "a section without page size or margins; A4 with 1 inch margins assumed",
            labels::DEFAULT_ASSUMED,
        ),
    ] {
        assert_eq!(kind(part, what), (Severity::Info, label), "{part}: {what}");
    }
}

#[test]
fn formatting_and_structure_are_minor() {
    for (part, what, label) in [
        (
            DOC,
            "border style approximated as single",
            labels::FORMAT_APPROXIMATED,
        ),
        (DOC, "w:rPr/w:spacing", labels::FORMAT_NOT_KEPT),
        (
            "styles.xml",
            "style:paragraph-properties@fo:hyphenation-ladder-count",
            labels::FORMAT_NOT_KEPT,
        ),
        (
            "styles.xml",
            "office:styles/text:outline-style",
            labels::FORMAT_NOT_KEPT,
        ),
        (
            "content.xml",
            "underline style wave read as single",
            labels::FORMAT_APPROXIMATED,
        ),
        (
            "content.xml",
            "a list item's further paragraphs read as items of their own",
            labels::STRUCTURE_SIMPLIFIED,
        ),
        (
            DOC,
            "field PAGEREF; its result is kept as text",
            labels::FIELD_AS_TEXT,
        ),
        (
            "images",
            "image type GIF not rendered",
            labels::IMAGE_PLACEHOLDER,
        ),
        (
            "markdown",
            "tracked insertion (accepted)",
            labels::TRACKED_CHANGES,
        ),
    ] {
        assert_eq!(kind(part, what), (Severity::Minor, label), "{part}: {what}");
    }
}

#[test]
fn lost_content_is_major() {
    for (part, what, label) in [
        (DOC, "a field that never ends", labels::CONTENT_NOT_KEPT),
        (DOC, "w:r/w:sym", labels::CONTENT_NOT_KEPT),
        (DOC, "w:customXml", labels::CONTENT_NOT_KEPT),
        (
            "content.xml",
            "draw:frame/draw:object",
            labels::IMAGE_NOT_KEPT,
        ),
        (
            "content.xml",
            "text:index-body between paragraphs",
            labels::CONTENT_NOT_KEPT,
        ),
        (
            DOC,
            "drawing that is not a picture (shape, chart or diagram)",
            labels::IMAGE_NOT_KEPT,
        ),
        (
            "word/charts/chart1.xml",
            "part not read",
            labels::PART_NOT_READ,
        ),
        ("markdown", "comment", labels::CONTENT_NOT_KEPT),
    ] {
        assert_eq!(kind(part, what), (Severity::Major, label), "{part}: {what}");
    }
}

/// The image sentences are built by helpers, not at a recording call, so the source scan
/// cannot see them: each builder's output is classified here.
#[test]
fn every_image_sentence_builder_is_named() {
    for what in [
        held_opaque("image/svg+xml"),
        ProbeError::Unsupported("WebP").not_rendered(),
        ProbeError::Malformed(ImageKind::Png).not_rendered(),
    ] {
        assert_eq!(
            kind("images", &what),
            (Severity::Minor, labels::IMAGE_PLACEHOLDER),
            "{what}"
        );
    }
}

#[test]
fn a_loss_the_table_does_not_name_is_major() {
    assert_eq!(
        kind(DOC, "something new went missing"),
        (Severity::Major, labels::UNCLASSIFIED)
    );
}

#[test]
fn a_pattern_value_is_one_word() {
    assert_eq!(
        kind(DOC, "a hyperlink without a target read as its text").0,
        Severity::Minor,
        "the exact sentence, not the \"{{x}} read as its text\" pattern"
    );
    assert_eq!(
        kind(DOC, "two words read as its text").1,
        labels::UNCLASSIFIED
    );
}

#[test]
fn reports_count_by_severity_and_carry_both_in_json() {
    let mut r = Report::empty();
    r.note_n("word/settings.xml", "w:settings/w:zoom", 2);
    r.note(DOC, "border style approximated as single");
    r.note_n(DOC, "a field that never ends", 3);
    r.macros_removed("word/vbaProject.bin", 10);
    assert_eq!(r.count_at_least(Severity::Info), 7);
    assert_eq!(r.count_at_least(Severity::Minor), 4);
    assert_eq!(r.count_at_least(Severity::Major), 3);
    let json: serde_json::Value = serde_json::to_value(&r).unwrap();
    let field = |i: usize, f: &str| json["losses"][i][f].as_str().unwrap().to_owned();
    assert_eq!(
        (field(0, "severity"), field(0, "label")),
        ("info".into(), labels::SETTINGS.into())
    );
    assert_eq!(field(2, "severity"), "major");
    assert_eq!(
        (field(3, "severity"), field(3, "label")),
        ("info".into(), labels::MACROS.into())
    );
}

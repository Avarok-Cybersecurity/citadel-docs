//! The corpus is deterministic (a golden fingerprint), its committed fixtures are what the
//! generator makes, and it lays out to the page counts it is named by.

use cdoc_bench::corpus::{
    fingerprint, generate, generate_seeded, keystroke_target, latin_text, to_json, typed,
    word_list, COMMITTED_PAGE_COUNTS, SEED,
};
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, BreakerChoice, LayoutOptions, Params};

/// The 10-page document's fingerprint. A change here changes every benchmark's input: it must be
/// deliberate, with the fixtures regenerated (`cdoc-bench corpus`) and the budgets re-read.
const GOLDEN_10P: u64 = 0x98c4_b7a3_85eb_b81a;

#[test]
fn the_ten_page_document_matches_its_golden_fingerprint() {
    assert_eq!(fingerprint(&generate(10)).unwrap(), GOLDEN_10P);
}

#[test]
fn generating_twice_gives_the_same_document() {
    assert_eq!(generate(10), generate(10));
}

#[test]
fn another_seed_gives_another_document() {
    let other = generate_seeded(10, SEED ^ 1);
    assert_ne!(fingerprint(&other).unwrap(), GOLDEN_10P);
}

#[test]
fn the_committed_fixtures_are_what_the_generator_makes() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus");
    for pages in COMMITTED_PAGE_COUNTS {
        let committed = std::fs::read_to_string(format!("{dir}/{pages}p.json")).unwrap();
        assert_eq!(
            committed,
            to_json(&generate(pages)).unwrap(),
            "{pages}p.json is stale"
        );
    }
}

fn page_count(pages: u32) -> usize {
    let doc = generate(pages);
    layout(
        &doc,
        &default_fonts(),
        &LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like())),
    )
    .unwrap()
    .pages
    .len()
}

#[test]
fn documents_lay_out_to_about_their_page_count() {
    assert_eq!(page_count(1), 1);
    let ten = page_count(10);
    assert!((9..=11).contains(&ten), "10-page document has {ten} pages");
    let hundred = page_count(100);
    assert!(
        (95..=105).contains(&hundred),
        "100-page document has {hundred} pages"
    );
}

#[test]
fn the_keystroke_lands_in_a_body_paragraph_in_the_second_half() {
    let doc = generate(10);
    let index = keystroke_target(&doc).unwrap();
    assert!(index >= doc.sections[0].blocks.len() / 2);
    let edited = typed(&doc, index, "x").unwrap();
    assert_ne!(edited, doc);
    assert_eq!(
        edited.sections[0].blocks.len(),
        doc.sections[0].blocks.len()
    );
}

#[test]
fn the_text_inputs_have_the_sizes_their_metrics_name() {
    assert_eq!(latin_text(10_000).chars().count(), 10_000);
    let words = word_list(2_000);
    assert_eq!(words.len(), 2_000);
    assert!(words.iter().all(|w| w.chars().all(char::is_alphabetic)));
}

//! The control: with every feature the model can switch off switched off (no ligatures, no
//! kerning, no hyphenation) and first-fit breaking, the integrated layout sets the frozen
//! fixture exactly as the greedy, unshaped layout before it did (`golden/frozen_greedy.txt`,
//! captured from that code and never regenerated). So shaping and the breaker change only
//! what they are meant to change.

mod frozen;
mod support;

use cdoc_layout::{layout, BreakerChoice, LayoutOptions};
use cdoc_model::{FontSize, Ligatures};

const FROZEN: &str = include_str!("golden/frozen_greedy.txt");

#[test]
fn with_features_off_and_first_fit_the_layout_is_the_pre_integration_one() {
    let mut doc = frozen::document();
    doc.settings.auto_hyphenation = false;
    doc.styles.defaults.run.ligatures = Some(Ligatures::None);
    doc.styles.defaults.run.kern = Some(FontSize(0));
    let options = LayoutOptions::word_like(BreakerChoice::FirstFit);
    let laid = layout(&doc, &support::fonts(), &options).expect("lays out");
    let dump = frozen::dump(&laid);
    let diff: Vec<String> = dump
        .lines()
        .zip(FROZEN.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("now  {a}\nwas  {b}"))
        .collect();
    assert!(
        diff.is_empty() && dump.lines().count() == FROZEN.lines().count(),
        "{} lines differ:\n{}",
        diff.len(),
        diff.join("\n")
    );
}

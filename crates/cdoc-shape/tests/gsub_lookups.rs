//! Every GSUB lookup type on a hand-built font, against hand-written expectations (and
//! rustybuzz, checked inside `shape`).

mod common;
mod support;

use cdoc_shape::{Feature, Features};
use support::fixture::{font, shape, simple, A, ALT, B, C, F, FI, I};
use support::layout::{chain, extension, ligatures, lookup, reverse, sequences, single, table};

fn gsub(feature: [u8; 4], lookups: Vec<support::layout::Lookup>) -> Vec<u8> {
    font(vec![(*b"GSUB", table(&[(&feature, &[0])], lookups))])
}

#[test]
fn single_substitution_replaces_each_covered_glyph() {
    let f = gsub(*b"ccmp", vec![lookup(1, vec![single(&[A], &[B])])]);
    let got = shape(&f, "abc", Features::word_default());
    assert_eq!(simple(&got), [(B, 0, 610), (B, 1, 610), (C, 2, 620)]);
}

#[test]
fn a_feature_switched_off_applies_nothing() {
    let f = gsub(
        *b"liga",
        vec![lookup(4, vec![ligatures(F, &[(&[F, I], FI)])])],
    );
    let off = Features::word_default().with(Feature::Liga, false);
    assert_eq!(simple(&shape(&f, "fi", off)), [(F, 0, 300), (I, 1, 250)]);
}

#[test]
fn multiple_substitution_splits_a_glyph_within_its_cluster() {
    let f = gsub(
        *b"ccmp",
        vec![lookup(2, vec![sequences(&[(FI, &[F, I]), (A, &[A, C])])])],
    );
    let got = shape(&f, "ab", Features::word_default());
    assert_eq!(simple(&got), [(A, 0, 600), (C, 0, 620), (B, 1, 610)]);
}

#[test]
fn alternate_substitution_takes_the_first_alternate() {
    let f = gsub(
        *b"ccmp",
        vec![lookup(3, vec![sequences(&[(A, &[ALT, C])])])],
    );
    assert_eq!(
        simple(&shape(&f, "a", Features::word_default())),
        [(ALT, 0, 660)]
    );
}

#[test]
fn ligature_substitution_merges_the_cluster() {
    let f = gsub(
        *b"liga",
        vec![lookup(4, vec![ligatures(F, &[(&[F, I], FI)])])],
    );
    let got = shape(&f, "fia", Features::word_default());
    assert_eq!(simple(&got), [(FI, 0, 550), (A, 2, 600)]);
}

#[test]
fn ligature_needs_every_component() {
    let f = gsub(
        *b"liga",
        vec![lookup(4, vec![ligatures(F, &[(&[F, I, I], FI)])])],
    );
    let got = shape(&f, "fi", Features::word_default());
    assert_eq!(simple(&got), [(F, 0, 300), (I, 1, 250)]);
}

#[test]
fn ligature_needs_the_right_components() {
    let f = gsub(
        *b"liga",
        vec![lookup(4, vec![ligatures(F, &[(&[F, I], FI)])])],
    );
    let got = shape(&f, "fa", Features::word_default());
    assert_eq!(simple(&got), [(F, 0, 300), (A, 1, 600)]);
}

#[test]
fn chained_context_applies_its_nested_lookup_only_in_context() {
    let lookups = vec![
        lookup(6, vec![chain(&[], &[&[A]], &[&[B]], &[(0, 1)])]),
        lookup(1, vec![single(&[A], &[ALT])]),
    ];
    let got = shape(&gsub(*b"calt", lookups), "abac", Features::word_default());
    assert_eq!(
        simple(&got),
        [(ALT, 0, 660), (B, 1, 610), (A, 2, 600), (C, 3, 620)]
    );
}

#[test]
fn chained_context_honours_backtrack() {
    let lookups = vec![
        lookup(6, vec![chain(&[&[C]], &[&[A]], &[], &[(0, 1)])]),
        lookup(1, vec![single(&[A], &[ALT])]),
    ];
    let got = shape(&gsub(*b"calt", lookups), "aca", Features::word_default());
    assert_eq!(simple(&got), [(A, 0, 600), (C, 1, 620), (ALT, 2, 660)]);
}

#[test]
fn extension_lookups_apply_what_they_wrap() {
    let wrapped = extension(4, ligatures(F, &[(&[F, I], FI)]));
    let got = shape(
        &gsub(*b"liga", vec![lookup(7, vec![wrapped])]),
        "fi",
        Features::word_default(),
    );
    assert_eq!(simple(&got), [(FI, 0, 550)]);
}

#[test]
fn reverse_chaining_substitutes_in_context() {
    let f = gsub(
        *b"calt",
        vec![lookup(8, vec![reverse(&[A], &[ALT], &[C], &[B])])],
    );
    let got = shape(&f, "cabaab", Features::word_default());
    let ids: Vec<u16> = got.iter().map(|g| g.id).collect();
    assert_eq!(ids, [C, ALT, B, A, A, B]);
}

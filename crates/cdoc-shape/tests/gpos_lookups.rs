//! Every GPOS lookup type, and the legacy `kern` table, on a hand-built font against
//! hand-written expectations (and rustybuzz, checked inside `shape`).

mod common;
mod support;

use cdoc_shape::{Feature, Features};
use support::fixture::{font, placed, shape, A, ACUTE, CAP_A, CLASSES, F, FI, GRAVE, I, V};
use support::gpos::{
    class_pairs, kern, mark_attach, mark_ligature, pairs, single, X_ADVANCE, X_PLACEMENT,
    Y_PLACEMENT,
};
use support::layout::{extension, gdef, ligatures, lookup, table, Lookup};

fn gpos(feature: [u8; 4], lookups: Vec<Lookup>) -> Vec<u8> {
    font(vec![
        (*b"GPOS", table(&[(&feature, &[0])], lookups)),
        (*b"GDEF", gdef(&CLASSES, &[])),
    ])
}

fn on() -> Features {
    Features::word_default()
}

#[test]
fn single_adjustment_moves_and_widens() {
    let f = gpos(
        *b"kern",
        vec![lookup(
            1,
            vec![single(
                &[A],
                X_PLACEMENT | Y_PLACEMENT | X_ADVANCE,
                &[10, 20, 50],
            )],
        )],
    );
    assert_eq!(
        placed(&shape(&f, "aV", on())),
        [(A, 650, 10, 20), (V, 640, 0, 0)]
    );
}

#[test]
fn pair_adjustment_by_glyph_kerns_only_the_pair() {
    let f = gpos(
        *b"kern",
        vec![lookup(
            2,
            vec![pairs(X_ADVANCE, 0, &[(V, CAP_A, &[-80], &[])])],
        )],
    );
    assert_eq!(
        placed(&shape(&f, "VAV", on())),
        [(V, 560, 0, 0), (CAP_A, 650, 0, 0), (V, 640, 0, 0)]
    );
}

#[test]
fn pair_adjustment_by_class_reads_the_matrix() {
    let sub = class_pairs(
        &[V, A],
        &[(V, V, 1)],
        &[(CAP_A, CAP_A, 1)],
        &[&[0, 0], &[0, -70]],
    );
    let f = gpos(*b"kern", vec![lookup(2, vec![sub])]);
    assert_eq!(
        placed(&shape(&f, "VAaA", on())),
        [
            (V, 570, 0, 0),
            (CAP_A, 650, 0, 0),
            (A, 600, 0, 0),
            (CAP_A, 650, 0, 0)
        ]
    );
}

#[test]
fn kerning_switched_off_applies_nothing() {
    let f = gpos(
        *b"kern",
        vec![lookup(
            2,
            vec![pairs(X_ADVANCE, 0, &[(V, CAP_A, &[-80], &[])])],
        )],
    );
    let off = on().with(Feature::Kern, false);
    assert_eq!(
        placed(&shape(&f, "VA", off)),
        [(V, 640, 0, 0), (CAP_A, 650, 0, 0)]
    );
}

#[test]
fn extension_lookups_position_what_they_wrap() {
    let wrapped = extension(2, pairs(X_ADVANCE, 0, &[(V, CAP_A, &[-80], &[])]));
    let f = gpos(*b"kern", vec![lookup(9, vec![wrapped])]);
    assert_eq!(
        placed(&shape(&f, "VA", on())),
        [(V, 560, 0, 0), (CAP_A, 650, 0, 0)]
    );
}

#[test]
fn mark_to_base_aligns_the_anchors_and_zeroes_the_mark() {
    let sub = mark_attach(&[ACUTE], &[A], &[(0, (100, 0))], &[&[(300, 700)]]);
    let f = gpos(*b"mark", vec![lookup(4, vec![sub])]);
    // x: base anchor 300 - mark anchor 100 - base advance 600.
    assert_eq!(
        placed(&shape(&f, "a\u{301}", on())),
        [(A, 600, 0, 0), (ACUTE, 0, -400, 700)]
    );
}

#[test]
fn mark_to_mark_stacks_on_the_previous_mark() {
    let base = mark_attach(
        &[ACUTE, GRAVE],
        &[A],
        &[(0, (100, 0)), (0, (100, 0))],
        &[&[(300, 700)]],
    );
    let stack = mark_attach(&[GRAVE], &[ACUTE], &[(0, (0, 0))], &[&[(100, 200)]]);
    let lookups = vec![lookup(4, vec![base]), lookup(6, vec![stack])];
    let f = font(vec![
        (
            *b"GPOS",
            table(&[(b"mark", &[0]), (b"mkmk", &[1])], lookups),
        ),
        (*b"GDEF", gdef(&CLASSES, &[])),
    ]);
    // Grave sits at the acute's anchor (100, 200) on top of the acute's own offset.
    let got = placed(&shape(&f, "a\u{301}\u{300}", on()));
    assert_eq!(
        got,
        [(A, 600, 0, 0), (ACUTE, 0, -400, 700), (GRAVE, 0, -300, 900)]
    );
}

#[test]
fn mark_to_ligature_attaches_to_its_component() {
    let liga = ligatures(F, &[(&[F, I], FI)]);
    let marks = mark_ligature(
        &[ACUTE],
        FI,
        &[(0, (0, 0))],
        &[&[(100, 700)], &[(400, 700)]],
    );
    let f = font(vec![
        (
            *b"GSUB",
            table(&[(b"liga", &[0])], vec![lookup(4, vec![liga])]),
        ),
        (
            *b"GPOS",
            table(&[(b"mark", &[0])], vec![lookup(5, vec![marks])]),
        ),
        (*b"GDEF", gdef(&CLASSES, &[])),
    ]);
    // The mark follows the second component: anchor 400 - ligature advance 550.
    assert_eq!(
        placed(&shape(&f, "fi\u{301}", on())),
        [(FI, 550, 0, 0), (ACUTE, 0, -150, 700)]
    );
}

#[test]
fn legacy_kern_applies_without_gpos_split_across_the_pair() {
    let f = font(vec![(*b"kern", kern(&[(V, CAP_A, -90)]))]);
    assert_eq!(
        placed(&shape(&f, "VAV", on())),
        [(V, 595, 0, 0), (CAP_A, 605, -45, 0), (V, 640, 0, 0)]
    );
}

#[test]
fn legacy_kern_is_ignored_when_gpos_exists() {
    let mut tables = vec![(*b"kern", kern(&[(V, CAP_A, -90)]))];
    tables.push((
        *b"GPOS",
        table(
            &[(b"kern", &[0])],
            vec![lookup(1, vec![single(&[I], X_ADVANCE, &[5])])],
        ),
    ));
    assert_eq!(
        placed(&shape(&font(tables), "VA", on())),
        [(V, 640, 0, 0), (CAP_A, 650, 0, 0)]
    );
}

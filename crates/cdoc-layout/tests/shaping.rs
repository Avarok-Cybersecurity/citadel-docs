//! Runs are set through the shaper with the features their properties ask for: a ligature is
//! one glyph standing for the characters of its cluster, and kerning moves a glyph's advance
//! by exactly the font's GPOS adjustment, scaled as the layout scales every advance.

mod support;

use cdoc_layout::{GlyphRun, Layout};
use cdoc_model::{FontSize, Ligatures, RunProperties};
use cdoc_shape::{scale_size, Direction, Face, Feature, Features, Language, Script, ShapeParams};

fn set(text: &str, properties: RunProperties) -> Layout {
    let block = support::para_with(
        Some(cdoc_model::builtin::ids::normal()),
        vec![cdoc_model::Inline::text(text, properties)],
    );
    support::laid_out(&support::doc_with(vec![block]))
}

fn first_run(layout: &Layout) -> &GlyphRun {
    layout.pages[0].runs().next().expect("a run")
}

/// Glyph advances of `text` in the regular face, in font units, as the shaper sets them.
fn units(text: &str, features: Features) -> Vec<i32> {
    let face = Face::parse(cdoc_layout::test_support::FACES[0].2, 0).expect("face");
    let params = ShapeParams {
        script: Script::Latin,
        language: Language::default_system(),
        direction: Direction::LeftToRight,
        features,
    };
    let run = cdoc_shape::Shaper::new(&face)
        .shape(text, &params)
        .expect("shapes");
    run.glyphs.iter().map(|g| g.x_advance).collect()
}

#[test]
fn a_ligature_is_one_glyph_carrying_the_clusters_of_its_characters() {
    let layout = set("office", RunProperties::none());
    let run = first_run(&layout);
    let clusters: Vec<u32> = run.glyphs.iter().map(|g| g.cluster).collect();
    assert_eq!(clusters, [0, 1, 4, 5], "o, ffi, c, e: {run:?}");
    assert_eq!(run.cluster_text(1), "ffi");
    assert_eq!(run.text, "office");

    let plain = set(
        "office",
        RunProperties {
            ligatures: Some(Ligatures::None),
            ..RunProperties::none()
        },
    );
    let clusters: Vec<u32> = first_run(&plain).glyphs.iter().map(|g| g.cluster).collect();
    assert_eq!(
        clusters,
        [0, 1, 2, 3, 4, 5],
        "w14:ligatures none sets each letter"
    );
}

#[test]
fn kerning_moves_the_advance_by_the_gpos_value() {
    let kerned = set("AV", RunProperties::none());
    let unkerned = set(
        "AV",
        RunProperties {
            kern: Some(FontSize(0)),
            ..RunProperties::none()
        },
    );
    let (k, u) = (first_run(&kerned), first_run(&unkerned));
    let on = units("AV", Features::word_default());
    let off = units("AV", Features::word_default().with(Feature::Kern, false));
    assert_ne!(on[0], off[0], "the face kerns AV");
    let per_em = Face::parse(cdoc_layout::test_support::FACES[0].2, 0)
        .expect("face")
        .units_per_em();
    let scaled = |v: i32| scale_size(i64::from(v), k.size, per_em);
    assert_eq!(k.glyphs[0].advance, scaled(on[0]));
    assert_eq!(u.glyphs[0].advance, scaled(off[0]));
    assert_eq!(
        k.glyphs[0].advance - u.glyphs[0].advance,
        scaled(on[0]) - scaled(off[0])
    );
}

#[test]
fn kerning_below_the_run_threshold_is_off() {
    let below = set(
        "AV",
        RunProperties {
            kern: Some(FontSize(48)),
            ..RunProperties::none()
        },
    );
    let off = units("AV", Features::word_default().with(Feature::Kern, false));
    let run = first_run(&below);
    let per_em = Face::parse(cdoc_layout::test_support::FACES[0].2, 0)
        .expect("face")
        .units_per_em();
    assert_eq!(
        run.glyphs[0].advance,
        scale_size(i64::from(off[0]), run.size, per_em),
        "w:kern 24pt does not kern 11pt text"
    );
}

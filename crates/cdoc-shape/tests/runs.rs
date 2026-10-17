//! The run API: scaling to hundredths of a point, cluster maps, carets and hit testing,
//! itemisation, plan reuse and the refusals.

mod common;
mod support;

use cdoc_shape::{
    itemise, scale, Direction, Face, Features, Glyph, Language, Script, ScriptRun, ShapeError,
    ShapedRun, Shaper,
};
use common::{params, CARLITO};
use support::fixture::{font, CLASSES, F, FI, I};
use support::layout::{gdef, ligatures, lookup, table};

fn fi_font(carets: &[(u16, &[i16])]) -> Vec<u8> {
    let liga = table(
        &[(b"liga", &[0])],
        vec![lookup(4, vec![ligatures(F, &[(&[F, I], FI)])])],
    );
    font(vec![(*b"GSUB", liga), (*b"GDEF", gdef(&CLASSES, carets))])
}

fn shaped<'a>(bytes: &'a [u8], text: &str) -> (Face<'a>, ShapedRun) {
    let face = Face::parse(bytes, 0).expect("face");
    let run = Shaper::new(&face)
        .shape(text, &params(Script::Latin, Features::word_default()))
        .expect("shapes");
    (face, run)
}

#[test]
fn scaling_rounds_half_away_from_zero() {
    // 1000 units at 11 pt (22 half points) in a 2048 em: 537.109... hundredths.
    assert_eq!(scale(1000, 22, 2048), 537);
    assert_eq!(scale(-1000, 22, 2048), -537);
    assert_eq!(scale(1, 20, 1000), 1); // 1.0 exactly
    assert_eq!(scale(1, 10, 1000), 1); // 0.5 rounds up
    assert_eq!(scale(-1, 10, 1000), -1); // -0.5 rounds down
    assert_eq!(scale(0, 22, 2048), 0);
}

#[test]
fn positions_scale_from_exact_sums_so_rounding_never_accumulates() {
    let glyph = |x_advance| Glyph {
        id: 1,
        cluster: 0,
        x_advance,
        y_advance: 0,
        x_offset: 0,
        y_offset: 0,
    };
    let run = ShapedRun {
        glyphs: vec![glyph(1); 4],
    };
    // Each glyph is 0.5 hundredths wide: rounding each and summing would give 2 per two glyphs.
    let xs: Vec<i32> = run.positions_pt100(10, 1000).iter().map(|p| p.0).collect();
    assert_eq!(xs, [0, 1, 1, 2]);
    assert_eq!(run.advance_pt100(10, 1000), 2);
}

#[test]
fn clusters_map_text_to_glyphs_both_ways() {
    let bytes = fi_font(&[]);
    let (_, run) = shaped(&bytes, "fif");
    let spans = run.clusters("fif");
    assert_eq!(spans.len(), 2);
    assert_eq!(
        (spans[0].text.clone(), spans[0].glyphs.clone()),
        (0..2, 0..1)
    );
    assert_eq!(
        (spans[1].text.clone(), spans[1].glyphs.clone()),
        (2..3, 1..2)
    );
    assert_eq!(run.glyph_for_byte("fif", 1), Some(0));
    assert_eq!(run.text_for_glyph("fif", 1), Some(2..3));
    assert_eq!(run.glyph_for_byte("fif", 3), None);
}

#[test]
fn ligature_carets_divide_evenly_without_gdef_carets() {
    let bytes = fi_font(&[]);
    let (face, run) = shaped(&bytes, "fi");
    assert_eq!(run.carets(&face, "fi"), [(0, 0), (1, 275), (2, 550)]);
}

#[test]
fn ligature_carets_come_from_gdef_when_present() {
    let bytes = fi_font(&[(FI, &[200])]);
    let (face, run) = shaped(&bytes, "fi");
    assert_eq!(face.ligature_carets(FI), Some(vec![200]));
    assert_eq!(run.carets(&face, "fi"), [(0, 0), (1, 200), (2, 550)]);
}

#[test]
fn hit_testing_picks_the_nearest_caret_the_earlier_on_a_tie() {
    let bytes = fi_font(&[(FI, &[200])]);
    let (face, run) = shaped(&bytes, "fi");
    assert_eq!(run.hit_test(&face, "fi", -50), 0);
    assert_eq!(run.hit_test(&face, "fi", 100), 0);
    assert_eq!(run.hit_test(&face, "fi", 101), 1);
    assert_eq!(run.hit_test(&face, "fi", 400), 2);
    assert_eq!(run.hit_test(&face, "fi", 9999), 2);
}

#[test]
fn itemisation_splits_by_script_and_weak_characters_join_the_run_before() {
    let text = "1 Ab, αβ ж\u{301}!";
    let runs = itemise(text);
    let expect = [
        (0..6, Script::Latin),
        (6..11, Script::Greek),
        (11..text.len(), Script::Cyrillic),
    ];
    let got: Vec<_> = runs.iter().map(|r| (r.range.clone(), r.script)).collect();
    assert_eq!(got, expect);
    assert_eq!(
        itemise("12 ."),
        [ScriptRun {
            range: 0..4,
            script: Script::Common
        }]
    );
    assert_eq!(itemise("aע")[1].script, Script::Unsupported);
    assert_eq!(itemise(""), []);
}

#[test]
fn refusals_are_errors_not_guesses() {
    let face = Face::parse(CARLITO, 0).expect("face");
    let mut shaper = Shaper::new(&face);
    let mut p = params(Script::Latin, Features::word_default());
    p.direction = Direction::RightToLeft;
    assert_eq!(shaper.shape("a", &p), Err(ShapeError::UnsupportedDirection));
    let p = params(Script::Unsupported, Features::word_default());
    assert_eq!(
        shaper.shape("ע", &p),
        Err(ShapeError::UnsupportedScript(Script::Unsupported))
    );
    assert_eq!(
        Face::parse(b"not a font", 0).err(),
        Some(ShapeError::Unreadable)
    );
}

#[test]
fn shaping_into_a_reused_run_matches_a_fresh_shape() {
    let face = Face::parse(CARLITO, 0).expect("face");
    let mut shaper = Shaper::new(&face);
    let p = params(Script::Latin, Features::word_default());
    let mut run = ShapedRun::empty();
    shaper
        .shape_into("a much longer first line", &p, &mut run)
        .expect("shapes");
    shaper.shape_into("To fit", &p, &mut run).expect("shapes");
    assert_eq!(run, Shaper::new(&face).shape("To fit", &p).expect("shapes"));
}

#[test]
fn locl_follows_the_language() {
    let face = Face::parse(CARLITO, 0).expect("face");
    let mut shaper = Shaper::new(&face);
    let mut p = params(Script::Latin, Features::word_default());
    let plain = shaper.shape("ş", &p).expect("shapes").glyphs[0].id;
    p.language = Language::opentype(*b"ROM ");
    let romanian = shaper.shape("ş", &p).expect("shapes").glyphs[0].id;
    assert_eq!(Some(plain), face.glyph('ş'));
    assert_eq!(Some(romanian), face.glyph('ș'));
}

#[test]
fn the_face_reports_its_metrics() {
    let face = Face::parse(CARLITO, 0).expect("face");
    assert_eq!(face.units_per_em(), 2048);
    let a = face.glyph('a').expect("a");
    assert!(face.advance(a) > 0);
    assert!(face.bounds(a).is_some());
    assert_eq!(face.glyph('\u{10FFFF}'), None);
    let m = face.vertical_metrics();
    assert!(m.ascender > 0 && m.descender < 0);
}

#[test]
fn word_defaults_make_the_fi_ligature() {
    let face = Face::parse(CARLITO, 0).expect("face");
    let run = Shaper::new(&face)
        .shape("fi", &params(Script::Latin, Features::word_default()))
        .expect("shapes");
    assert_eq!(run.glyphs.len(), 1);
    assert_eq!(Some(run.glyphs[0].id), face.glyph('\u{FB01}'));
}

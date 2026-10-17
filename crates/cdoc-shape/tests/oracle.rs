//! The oracle: for random Latin, Greek and Cyrillic text (ligature triggers, kerning pairs,
//! precomposed and combining marks, joiners and soft hyphens) and random feature sets, every
//! glyph id, cluster, advance and offset equals rustybuzz's, on every fixture face, run by run
//! as `itemise` splits the text.

mod common;

use cdoc_shape::{itemise, Feature, Features, Script, ShapeError, Shaper};
use common::{faces, ours, params, rustybuzz};
use proptest::prelude::*;

const LATIN: &[&str] = &[
    "a", "b", "c", "e", "f", "g", "i", "j", "l", "n", "o", "r", "s", "t", "y", "A", "F", "L", "P",
    "T", "V", "W", "Y", "fi", "fl", "ff", "ffi", "ffl", "AV", "To", "Ta", "Te", "Yo", "LT", "P.",
    "Wa", "é", "ü", "ñ", "ç", "ø", "å", "ł", "ő", "ș", "ț", "ạ", "ệ", "ữ", "ḍ", "ǆ", "ŉ", "Ǻ", "ı",
    "ĳ", "ﬁ",
];
const GREEK: &[&str] = &[
    "α", "β", "γ", "ο", "ς", "ω", "Α", "Τ", "Υ", "ά", "έ", "ή", "ΐ", "ἀ", "ᾶ", "ῷ", "Ὦ", "ϊ",
];
const CYRILLIC: &[&str] = &[
    "а", "б", "г", "д", "о", "т", "я", "А", "Г", "Т", "У", "ё", "ї", "ґ", "й", "Ў", "ӱ",
];
const COMMON: &[&str] = &[
    " ", " ", "1", "7", ".", ",", ";", "!", "?", "-", "–", "—", "'", "\"", "“", "”", "(", ")", "/",
    "\u{A0}", "\u{AD}", "\u{200B}", "\u{200C}", "\u{200D}", "\u{2009}", "\u{2003}", "\u{2011}",
];
const MARKS: &[&str] = &[
    "\u{300}", "\u{301}", "\u{302}", "\u{303}", "\u{304}", "\u{306}", "\u{307}", "\u{308}",
    "\u{30A}", "\u{30B}", "\u{30C}", "\u{313}", "\u{314}", "\u{323}", "\u{327}", "\u{328}",
    "\u{331}", "\u{342}", "\u{345}", "\u{34F}", "\u{1DC4}",
];

fn token() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        4 => proptest::sample::select(LATIN),
        2 => proptest::sample::select(GREEK),
        2 => proptest::sample::select(CYRILLIC),
        2 => proptest::sample::select(COMMON),
        2 => proptest::sample::select(MARKS),
    ]
}

fn text() -> impl Strategy<Value = String> {
    proptest::collection::vec(token(), 1..40).prop_map(|t| t.concat())
}

/// Any character of the repertoire's blocks, assigned or not, drawable or not, except the marks
/// Unicode 17.0 added at U+1ACF..U+1AEB: our tables are Unicode 16.0, and so are rustybuzz's
/// combining classes and compositions, but its general categories are 17.0, so for those code
/// points the oracle disagrees with itself.
fn wild() -> impl Strategy<Value = String> {
    let ranges = prop_oneof![
        3 => 0x20_u32..0x250,
        2 => 0x300_u32..0x370,
        2 => 0x370_u32..0x530,
        1 => 0x1AB0_u32..0x1B00,
        1 => 0x1DC0_u32..0x1E00,
        3 => 0x1E00_u32..0x2000,
        1 => 0x2000_u32..0x2070,
        1 => 0x20D0_u32..0x2100,
        1 => 0x2100_u32..0x2300,
        1 => 0xFB00_u32..0xFB07,
        1 => 0xFE00_u32..0xFE30,
        1 => 0xFF00_u32..0xFFF0,
        1 => 0x1_F1E6_u32..0x1_F200,
        1 => 0x1_F300_u32..0x1_F700,
        1 => 0xE_0000_u32..0xE_0080,
        1 => 0x0590_u32..0x0700,
    ];
    let assigned_in_16 = |c: &u32| !(0x1ACF..=0x1AEB).contains(c);
    let chars = ranges
        .prop_filter("Unicode 16", assigned_in_16)
        .prop_filter_map("scalar", char::from_u32);
    proptest::collection::vec(chars, 1..40).prop_map(|cs| cs.into_iter().collect())
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(512)
}

fn features() -> impl Strategy<Value = Features> {
    proptest::collection::vec(any::<bool>(), Feature::ALL.len()).prop_map(|on| {
        Feature::ALL
            .iter()
            .zip(on)
            .fold(Features::none(), |f, (&x, on)| f.with(x, on))
    })
}

fn agree(text: &str, features: Features) -> Result<(), TestCaseError> {
    for (name, face, rb) in faces() {
        for run in itemise(text) {
            let slice = &text[run.range.clone()];
            if run.script == Script::Unsupported {
                let refused = Shaper::new(face).shape(slice, &params(run.script, features));
                prop_assert_eq!(
                    refused,
                    Err(ShapeError::UnsupportedScript(Script::Unsupported))
                );
                continue;
            }
            let p = params(run.script, features);
            let (a, b) = (ours(face, slice, p), rustybuzz(rb, slice, p));
            prop_assert_eq!(
                &a,
                &b,
                "{} on {:?} ({:?}, {:?})",
                name,
                slice,
                run.script,
                features
            );
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), ..ProptestConfig::default() })]

    #[test]
    fn every_glyph_matches_rustybuzz(text in text(), features in features()) {
        agree(&text, features)?;
    }

    #[test]
    fn any_character_of_the_repertoire_matches_rustybuzz(text in wild(), features in features()) {
        agree(&text, features)?;
    }

    #[test]
    fn every_glyph_matches_rustybuzz_with_word_defaults(text in text()) {
        agree(&text, Features::word_default())?;
    }
}

#[test]
fn a_paragraph_of_each_script_matches() {
    let samples = [
        "The office’s first affidavit: “AVAST, To Ta!” — fluffy waffles, 1,234.56 €.",
        "Ἐν ἀρχῇ ἦν ὁ λόγος, καὶ ὁ λόγος ἦν πρὸς τὸν θεόν. Τὰ ΑΥΤΑ.",
        "Съешь же ещё этих мягких французских булок, да выпей чаю. ТУТ ГАТ.",
        "ạ̈ é̃ x̧̌ o\u{323}\u{302}\u{301} ǖ e\u{301}\u{301} A\u{30A} \u{212B} ﬁ\u{301}",
    ];
    for text in samples {
        agree(text, Features::word_default()).expect("agrees");
        agree(text, Features::none()).expect("agrees");
    }
}

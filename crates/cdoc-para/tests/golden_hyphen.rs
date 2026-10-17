//! Liang hyphenation against TeX itself. INITEX loaded each shipped pattern file unmodified and
//! `\showhyphens` each word with the file's typesetting hyphen minimums
//! (`tests/oracle/hyphen.py` wrote `golden/hyphen.rs`). The en-US list holds the classic words
//! (hyphenation, associate, project, table, concatenation, supercalifragilisticexpialidocious,
//! the `TeXbook`'s exception words) plus the Frog King's.

#[path = "golden/hyphen.rs"]
mod hyphen;

use cdoc_para::{HyphenMins, Hyphenator, Language, PatternError};

fn shown(word: &str, breaks: &[usize]) -> String {
    let mut out = String::new();
    let mut last = 0;
    for &b in breaks {
        out.push_str(&word[last..b]);
        out.push('-');
        last = b;
    }
    out.push_str(&word[last..]);
    out
}

fn check(language: Language, expected: &[&str]) {
    let h = Hyphenator::for_language(language).unwrap();
    let mut buf = Vec::new();
    for want in expected {
        let word = want.replace('-', "");
        assert_eq!(shown(&word, &h.hyphenate(&word)), *want, "{language:?}");
        h.hyphenate_into(&word, &mut buf);
        assert_eq!(shown(&word, &buf), *want, "{language:?} (into)");
    }
}

#[test]
fn en_us_matches_tex() {
    check(Language::EnUs, hyphen::EN_US);
}

#[test]
fn en_gb_matches_tex() {
    check(Language::EnGb, hyphen::EN_GB);
}

#[test]
fn de_1996_matches_tex() {
    check(Language::De1996, hyphen::DE_1996);
}

#[test]
fn fr_matches_tex() {
    check(Language::Fr, hyphen::FR);
}

#[test]
fn es_matches_tex() {
    check(Language::Es, hyphen::ES);
}

#[test]
fn the_classic_words_are_in_the_golden_list() {
    for w in [
        "hy-phen-ation",
        "as-so-ciate",
        "project",
        "ta-ble",
        "con-cate-na-tion",
        "su-per-cal-ifrag-ilis-tic-ex-pi-ali-do-cious",
    ] {
        assert!(hyphen::EN_US.contains(&w), "{w}");
    }
}

#[test]
fn capitals_hyphenate_as_lowercase() {
    let h = Hyphenator::for_language(Language::EnUs).unwrap();
    assert_eq!(
        shown("Hyphenation", &h.hyphenate("Hyphenation")),
        "Hy-phen-ation"
    );
    assert_eq!(shown("TABLE", &h.hyphenate("TABLE")), "TA-BLE");
    let de = Hyphenator::for_language(Language::De1996).unwrap();
    let word = "Übergrößenträger";
    assert_eq!(shown(word, &de.hyphenate(word)), "Über-grö-ßen-trä-ger");
}

#[test]
fn each_language_takes_its_files_minimums() {
    let mins = |l| Hyphenator::for_language(l).unwrap().mins();
    assert_eq!(mins(Language::EnUs), HyphenMins::new(2, 3).unwrap());
    assert_eq!(mins(Language::EnGb), HyphenMins::new(2, 3).unwrap());
    for l in [Language::De1996, Language::Fr, Language::Es] {
        assert_eq!(mins(l), HyphenMins::new(2, 2).unwrap(), "{l:?}");
    }
}

#[test]
fn hyphen_minimums_hold_for_patterns_and_exceptions() {
    let source = "\\patterns{1a 1b 1c 1d 1e 1f}\n\\hyphenation{a-b-c-d-e-f-g}";
    let h = |l, r| Hyphenator::from_patterns(source, HyphenMins::new(l, r).unwrap()).unwrap();
    assert_eq!(
        shown("abcdefg", &h(1, 1).hyphenate("abcdefg")),
        "a-b-c-d-e-f-g"
    );
    assert_eq!(
        shown("abcdefg", &h(2, 3).hyphenate("abcdefg")),
        "ab-c-d-efg"
    );
    assert_eq!(shown("bcdef", &h(1, 1).hyphenate("bcdef")), "b-c-d-e-f");
    assert_eq!(shown("bcdef", &h(2, 2).hyphenate("bcdef")), "bc-d-ef");
    assert!(
        h(3, 3).hyphenate("abcde").is_empty(),
        "shorter than left + right"
    );
}

#[test]
fn the_last_exception_for_a_word_wins() {
    let source = "\\patterns{a1b}\\hyphenation{ab-cd abc-d}";
    let h = Hyphenator::from_patterns(source, HyphenMins::new(1, 1).unwrap()).unwrap();
    assert_eq!(shown("abcd", &h.hyphenate("abcd")), "abc-d");
}

#[test]
fn words_beyond_tex_s_limit_are_left_whole() {
    let h = Hyphenator::for_language(Language::EnUs).unwrap();
    let long = "hyphenation".repeat(6);
    assert!(long.chars().count() > cdoc_para::MAX_WORD);
    assert_eq!(h.hyphenate(&long), Vec::<usize>::new());
    let fits = "hyphenation".repeat(5);
    assert_ne!(h.hyphenate(&fits), Vec::<usize>::new());
}

#[test]
fn malformed_sources_are_errors() {
    let mins = HyphenMins::new(2, 3).unwrap();
    let err = |s: &str| Hyphenator::from_patterns(s, mins).unwrap_err();
    assert!(matches!(
        err("\\patterns{a12b}"),
        PatternError::Malformed { line: 1, .. }
    ));
    assert!(matches!(
        err("\\patterns{\n1a\n.}"),
        PatternError::Malformed { line: 3, .. }
    ));
    assert!(matches!(
        err("\\patterns{a.b}"),
        PatternError::Malformed { .. }
    ));
    assert!(matches!(
        err("\\patterns{1a a1}"),
        PatternError::Duplicate { .. }
    ));
    assert!(matches!(
        err("\\patterns{1a"),
        PatternError::Unclosed { .. }
    ));
    assert!(matches!(
        err("\\lccode{1a}"),
        PatternError::Unsupported { .. }
    ));
    assert!(matches!(
        err("% only a comment\n"),
        PatternError::NoPatterns
    ));
    assert!(matches!(
        err("\\patterns{1a}\\hyphenation{ab1c}"),
        PatternError::Malformed { .. }
    ));
    assert_eq!(HyphenMins::new(0, 2), Err(PatternError::ZeroHyphenMin));
}

#[test]
fn every_shipped_file_keeps_its_licence_header() {
    for l in Language::ALL {
        let text = l.patterns();
        assert!(text.contains("% licence:"), "{l:?}");
        assert!(text.contains("% copyright:"), "{l:?}");
    }
    assert!(Language::EnUs
        .patterns()
        .contains("Copying and distribution of this file, with or without modification,"));
}

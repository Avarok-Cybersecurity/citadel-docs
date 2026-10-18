//! Hyphenation as Word controls it: `w:autoHyphenation` turns pattern hyphenation on, a
//! paragraph's `w:suppressAutoHyphens` turns it off there, the run's language (else the
//! document's) picks the pattern set, a language without patterns is reported, and a soft
//! hyphen is a break that shows only when a line ends at it.

mod support;

use cdoc_layout::{Dirty, Layout, LayoutSession, Warning};
use cdoc_model::{Alignment, Block, Document, Indent, Length};

const WORDS: &str = "International responsibilities characteristically accompany \
    extraordinary administrative considerations, notwithstanding unquestionably \
    incomprehensible institutional rearrangements and correspondingly multidimensional \
    representations of interdisciplinary organisational understanding.";

/// Words whose patterns allow a break two letters from the end (`-ly`), which only the right
/// minimum forbids.
const ADVERBS: &str = "Unquestionably, correspondingly and accordingly, sporadically and \
    systematically, the administrators reluctantly, hurriedly and unexpectedly reconsidered \
    increasingly and disproportionately complicated arrangements, approximately, separately \
    and independently.";

/// A narrow justified column of `text` with `suppress` set, in a document whose automatic
/// hyphenation is `auto` and whose language is `language`.
fn document(text: &str, auto: bool, suppress: Option<bool>, language: Option<&str>) -> Document {
    document_at(text, auto, suppress, language, 14400)
}

/// As [`document`], `inset` in from both margins.
fn document_at(
    text: &str,
    auto: bool,
    suppress: Option<bool>,
    language: Option<&str>,
    inset: i32,
) -> Document {
    let mut block = support::para(text);
    if let Block::Paragraph(p) = &mut block {
        p.properties.alignment = Some(Alignment::Justify);
        p.properties.suppress_auto_hyphens = suppress;
        p.properties.indent = Some(Indent {
            left: Some(Length(inset)),
            right: Some(Length(inset)),
            first_line: None,
        });
    }
    let mut doc = support::doc_with(vec![block]);
    doc.settings.auto_hyphenation = auto;
    doc.meta.language = language.map(str::to_owned);
    doc
}

/// Each line's text, its runs joined.
fn lines(layout: &Layout) -> Vec<String> {
    let mut out: Vec<(i32, String)> = Vec::new();
    for run in layout.pages[0].runs() {
        match out.last_mut() {
            Some((y, text)) if *y == run.y => text.push_str(&run.text),
            _ => out.push((run.y, run.text.clone())),
        }
    }
    out.into_iter().map(|(_, t)| t).collect()
}

/// Every break inside a word: the word (letters only) and how many letters precede the break.
fn breaks(lines: &[String]) -> Vec<(String, usize)> {
    let letters = |s: &str| s.chars().filter(|c| c.is_alphabetic()).collect::<String>();
    lines
        .windows(2)
        .filter_map(|pair| {
            let head = pair[0].strip_suffix('-')?;
            let before = letters(head.rsplit(' ').next().unwrap_or(""));
            let after = letters(pair[1].split(' ').next().unwrap_or(""));
            Some((format!("{before}{after}"), before.chars().count()))
        })
        .collect()
}

fn laid(doc: &Document) -> (Vec<String>, Layout) {
    let layout = support::laid_out(doc);
    (lines(&layout), layout)
}

#[test]
fn automatic_hyphenation_breaks_words_and_suppress_auto_hyphens_stops_it() {
    let (on, _) = laid(&document(WORDS, true, None, Some("en-US")));
    assert!(!breaks(&on).is_empty(), "nothing hyphenated: {on:#?}");
    let (suppressed, _) = laid(&document(WORDS, true, Some(true), Some("en-US")));
    assert!(breaks(&suppressed).is_empty(), "{suppressed:#?}");
    let (off, _) = laid(&document(WORDS, false, None, Some("en-US")));
    assert!(breaks(&off).is_empty(), "{off:#?}");
}

/// `hyph-en-us.tex` declares `left: 2, right: 3` for typesetting, `hyph-de-1996.tex` 2 and 2.
/// Columns of many widths, so that every break the patterns offer is taken somewhere.
#[test]
fn every_break_leaves_the_pattern_files_minimum_letters_either_side() {
    for (tag, left, right) in [("en-US", 2, 3), ("de", 2, 2)] {
        let mut found = Vec::new();
        for inset in (12000..=19000).step_by(250) {
            for text in [WORDS, ADVERBS] {
                let (lines, _) = laid(&document_at(text, true, None, Some(tag), inset));
                found.extend(breaks(&lines));
            }
        }
        assert!(found.len() > 50, "{tag}: only {} breaks", found.len());
        for (word, at) in found {
            let n = word.chars().count();
            assert!(
                at >= left && n - at >= right,
                "{tag}: {word:?} broken after {at} of {n} letters"
            );
        }
    }
}

#[test]
fn the_language_picks_the_pattern_set() {
    for (tag, language) in [
        ("en-US", cdoc_para::Language::EnUs),
        ("de", cdoc_para::Language::De1996),
    ] {
        let patterns = cdoc_para::Hyphenator::for_language(language).expect("compiles");
        let (lines, _) = laid(&document(WORDS, true, None, Some(tag)));
        for (word, at) in breaks(&lines) {
            let points: Vec<usize> = patterns
                .hyphenate(&word.to_lowercase())
                .into_iter()
                .map(|byte| word[..byte].chars().count())
                .collect();
            assert!(
                points.contains(&at),
                "{tag}: {word:?} at {at}, not in {points:?}"
            );
        }
    }
    let us = breaks(&laid(&document(WORDS, true, None, Some("en-US"))).0);
    let de = breaks(&laid(&document(WORDS, true, None, Some("de"))).0);
    assert_ne!(us, de, "the two pattern sets break the column differently");
}

#[test]
fn a_language_without_patterns_is_reported_and_not_hyphenated() {
    let (lines, layout) = laid(&document(WORDS, true, None, Some("tlh")));
    assert!(breaks(&lines).is_empty(), "{lines:#?}");
    assert!(
        layout.warnings.contains(&Warning::NoHyphenationPatterns {
            language: "tlh".to_owned()
        }),
        "{:?}",
        layout.warnings
    );
}

#[test]
fn a_soft_hyphen_shows_only_where_a_line_ends_at_it() {
    let text = "extra\u{AD}ordinary con\u{AD}cate\u{AD}nation ".repeat(8);
    let (lines, layout) = laid(&document(&text, false, None, Some("en-US")));
    let hyphen = layout.pages[0]
        .runs()
        .find(|r| r.text.ends_with('-'))
        .and_then(|r| r.glyphs.last())
        .expect("a line ends at a soft hyphen")
        .id;
    let mut unseen = 0;
    for run in layout.pages[0].runs() {
        for (i, g) in run.glyphs.iter().enumerate() {
            let soft = run.cluster_text(i) == "\u{AD}";
            unseen += usize::from(soft);
            assert!(
                !soft || (g.advance == 0 && g.id != hyphen),
                "a soft hyphen mid-line draws {g:?} in {:?}",
                run.text
            );
        }
    }
    assert!(unseen > 0, "no soft hyphen stayed mid-line");
    let ended = lines.iter().filter(|l| l.ends_with('-')).count();
    assert!(ended > 0, "no line ended at a soft hyphen: {lines:#?}");
    for line in &lines {
        let body = line.strip_suffix('-').unwrap_or(line);
        assert!(!body.contains('-'), "a hyphen shows mid-line: {line:?}");
    }
    let squeeze = |s: &str| s.replace(['\u{AD}', ' '], "");
    let rejoined: String = lines
        .iter()
        .map(|l| squeeze(l.strip_suffix('-').unwrap_or(l)))
        .collect();
    assert_eq!(
        rejoined,
        squeeze(&text),
        "the text survives, soft hyphens unseen"
    );
}

/// The language is in the measure cache's key: a paragraph measured under one pattern set is
/// broken again under the other, as a fresh layout breaks it.
#[test]
fn a_relayout_after_the_language_changes_is_a_fresh_layout() {
    let (fonts, options) = (support::fonts(), support::options());
    let mut session = LayoutSession::new(&fonts, &options);
    let us = document(WORDS, true, None, Some("en-US"));
    session.relayout(&us, &Dirty::all()).expect("lays out");
    let de = document(WORDS, true, None, Some("de"));
    let dirty = Dirty {
        typesetting_changed: true,
        ..Dirty::edits(Vec::new())
    };
    let got = session.relayout(&de, &dirty).expect("relays out");
    assert_eq!(got.layout(), &support::laid_out(&de));
}

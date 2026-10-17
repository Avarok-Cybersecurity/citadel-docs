//! Liang's hyphenation, as TeX does it: the word's letters, lowercased and framed by `.`, are
//! matched against every pattern; the highest digit at each gap wins and odd digits are
//! hyphens. An exception list overrides the patterns for whole words, and `\lefthyphenmin` /
//! `\righthyphenmin` keep hyphens away from the word's ends (exceptions included, as in TeX).

mod language;
mod parse;
mod trie;

pub use language::Language;

use crate::error::PatternError;
use parse::{header_mins, parse};
use trie::{Pattern, Trie};

/// TeX's longest hyphenatable word, in letters; longer words are left whole.
pub const MAX_WORD: usize = 63;

/// `\lefthyphenmin` and `\righthyphenmin`: the fewest letters before the first hyphen and
/// after the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HyphenMins {
    left: usize,
    right: usize,
}

impl HyphenMins {
    /// Both must be at least 1.
    pub fn new(left: usize, right: usize) -> Result<Self, PatternError> {
        if left == 0 || right == 0 {
            return Err(PatternError::ZeroHyphenMin);
        }
        Ok(Self { left, right })
    }

    /// Letters before the first hyphen.
    #[must_use]
    pub const fn left(&self) -> usize {
        self.left
    }

    /// Letters after the last hyphen.
    #[must_use]
    pub const fn right(&self) -> usize {
        self.right
    }
}

/// TeX's `\lccode` for letters: the lowercase form when it is one character, else the letter.
pub(crate) fn lower(c: char) -> char {
    let mut l = c.to_lowercase();
    match (l.next(), l.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// An exception's letters and the letter counts before its hyphens.
type Entry = (Box<[char]>, Box<[usize]>);

/// A compiled pattern set with its exceptions and hyphen minimums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyphenator {
    trie: Trie,
    /// Sorted by letters; the last entry for a word wins, as in TeX.
    exceptions: Vec<Entry>,
    mins: HyphenMins,
}

impl Hyphenator {
    /// Compiles a TeX pattern file's text.
    pub fn from_patterns(source: &str, mins: HyphenMins) -> Result<Self, PatternError> {
        let parsed = parse(source)?;
        let patterns: Vec<Pattern> = parsed.patterns.iter().map(|(p, _)| p.clone()).collect();
        let trie = Trie::build(&patterns).map_err(|k| {
            let (p, line) = &parsed.patterns[k];
            let mut pattern = String::new();
            for (c, v) in p.letters.iter().zip(&p.values) {
                if *v > 0 {
                    pattern.push(char::from(b'0' + v));
                }
                pattern.push(*c);
            }
            PatternError::Duplicate {
                line: *line,
                pattern,
            }
        })?;
        let mut exceptions: Vec<Entry> = Vec::new();
        for (letters, hyphens) in parsed.exceptions.into_iter().rev() {
            if !exceptions.iter().any(|(l, _)| **l == *letters) {
                exceptions.push((letters.into(), hyphens.into()));
            }
        }
        exceptions.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Self {
            trie,
            exceptions,
            mins,
        })
    }

    /// The shipped patterns for `language`, with the hyphen minimums its file declares for
    /// typesetting.
    pub fn for_language(language: Language) -> Result<Self, PatternError> {
        let source = language.patterns();
        Self::from_patterns(source, header_mins(source)?)
    }

    /// The hyphen minimums in force.
    #[must_use]
    pub const fn mins(&self) -> HyphenMins {
        self.mins
    }

    /// The byte offsets in `word` before which a hyphen may go, ascending.
    #[must_use]
    pub fn hyphenate(&self, word: &str) -> Vec<usize> {
        let mut out = Vec::new();
        self.hyphenate_into(word, &mut out);
        out
    }

    /// As [`Hyphenator::hyphenate`], into `out` (cleared first). Allocates nothing once `out`
    /// has the capacity: the word is worked on in fixed buffers of [`MAX_WORD`] letters.
    pub fn hyphenate_into(&self, word: &str, out: &mut Vec<usize>) {
        out.clear();
        let mut text = ['.'; MAX_WORD + 2];
        let mut offsets = [0usize; MAX_WORD];
        let mut n = 0;
        for (offset, c) in word.char_indices() {
            if n == MAX_WORD {
                return;
            }
            text[n + 1] = lower(c);
            offsets[n] = offset;
            n += 1;
        }
        let (left, right) = (self.mins.left, self.mins.right);
        if n < left + right {
            return;
        }
        text[n + 1] = '.';
        let letters = &text[1..=n];
        let found = self
            .exceptions
            .binary_search_by(|(l, _)| (**l).cmp(letters));
        if let Ok(k) = found {
            let hyphens = &self.exceptions[k].1;
            out.extend(
                hyphens
                    .iter()
                    .filter(|&&p| p >= left && p <= n - right)
                    .map(|&p| offsets[p]),
            );
            return;
        }
        let mut vals = [0u8; MAX_WORD + 3];
        let framed = &text[..n + 2];
        for start in 0..framed.len() {
            self.trie.apply(framed, start, &mut vals);
        }
        out.extend(
            (left..=n - right)
                .filter(|&p| vals[p + 1] % 2 == 1)
                .map(|p| offsets[p]),
        );
    }
}

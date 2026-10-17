//! Prose from a fixed vocabulary: sentences of varied length, sometimes with a clause, so line
//! breaking and hyphenation meet realistic word lengths rather than one repeated paragraph.

use crate::prng::SplitMix64;

const WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "a",
    "to",
    "in",
    "is",
    "that",
    "for",
    "it",
    "as",
    "with",
    "was",
    "on",
    "be",
    "by",
    "this",
    "are",
    "from",
    "or",
    "which",
    "an",
    "not",
    "at",
    "have",
    "its",
    "their",
    "can",
    "more",
    "one",
    "all",
    "each",
    "when",
    "there",
    "been",
    "into",
    "only",
    "other",
    "than",
    "most",
    "document",
    "paragraph",
    "section",
    "measure",
    "typesetter",
    "editor",
    "printer",
    "page",
    "column",
    "margin",
    "heading",
    "footnote",
    "table",
    "border",
    "spacing",
    "indent",
    "justified",
    "hyphenation",
    "ligature",
    "kerning",
    "baseline",
    "leading",
    "glyph",
    "outline",
    "character",
    "language",
    "dictionary",
    "revision",
    "comment",
    "collaboration",
    "workspace",
    "encrypted",
    "peer",
    "session",
    "network",
    "protocol",
    "signature",
    "archive",
    "export",
    "import",
    "careful",
    "consistent",
    "deterministic",
    "incremental",
    "independent",
    "reasonable",
    "considerable",
    "extraordinary",
    "uncomplicated",
    "responsibility",
    "interpretation",
    "organisation",
    "administration",
    "requirement",
    "performance",
    "benchmark",
    "competitor",
    "measurement",
    "throughput",
    "latency",
    "keystroke",
    "reflow",
    "pagination",
    "widow",
    "orphan",
    "numbering",
    "bullet",
    "quotation",
    "emphasis",
    "office",
    "room",
    "member",
    "owner",
    "release",
    "budget",
    "regression",
    "window",
    "browser",
    "worker",
    "thread",
    "memory",
    "allocation",
    "buffer",
    "stream",
    "cursor",
    "selection",
    "clipboard",
    "undo",
    "history",
    "snapshot",
    "version",
    "compatibility",
    "fidelity",
    "rendering",
    "preview",
    "resolution",
    "accessibility",
    "contrast",
    "landscape",
    "portrait",
    "binding",
    "quickly",
    "slowly",
    "carefully",
    "exactly",
    "nearly",
    "always",
    "never",
    "often",
    "rarely",
    "together",
    "between",
    "through",
    "without",
    "against",
    "beneath",
    "across",
    "around",
    "before",
    "after",
    "during",
    "because",
];

const CONNECTIVES: &[&str] = &[
    "however",
    "therefore",
    "meanwhile",
    "moreover",
    "instead",
    "still",
];

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `count` words, space-separated, the first capitalised when `title` is set for every word.
pub fn phrase(rng: &mut SplitMix64, count: usize, title: bool) -> String {
    let mut out = String::new();
    for k in 0..count {
        if k > 0 {
            out.push(' ');
        }
        let word = WORDS[rng.below(WORDS.len())];
        if title || k == 0 {
            out.push_str(&capitalised(word));
        } else {
            out.push_str(word);
        }
    }
    out
}

/// One sentence of 6 to 22 words, sometimes with a connective clause, ending in a full stop.
pub fn sentence(rng: &mut SplitMix64) -> String {
    let count = rng.between(6, 22);
    let mut s = phrase(rng, count, false);
    if rng.below(4) == 0 {
        s.push_str(", ");
        s.push_str(CONNECTIVES[rng.below(CONNECTIVES.len())]);
        s.push(' ');
        let tail = rng.between(3, 8);
        s.push_str(&phrase(rng, tail, false).to_lowercase());
    }
    s.push('.');
    s
}

/// `low..=high` sentences joined by spaces.
pub fn sentences(rng: &mut SplitMix64, low: usize, high: usize) -> String {
    let count = rng.between(low, high);
    (0..count)
        .map(|_| sentence(rng))
        .collect::<Vec<_>>()
        .join(" ")
}

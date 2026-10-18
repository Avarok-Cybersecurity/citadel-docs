//! Hyphenation for the layout: the hyphenation points of a word under the patterns the font
//! port registered for its language (compiled once, when registered), with a reused buffer.

use cdoc_para::Hyphenator;

/// The working buffer of [`Hyphenators::points`], kept between paragraphs and relayouts.
#[derive(Debug, Clone)]
pub struct Hyphenators {
    points: Vec<usize>,
}

impl Hyphenators {
    /// An empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// The byte offsets in `word` before which the patterns allow a hyphen, ascending. Only
    /// runs of letters are hyphenated (each on its own, as TeX takes a word to end at the
    /// first character that is not a letter), so `word,` and `(word)` hyphenate as `word`.
    pub fn points(&mut self, hyphenator: &Hyphenator, word: &str, out: &mut Vec<usize>) {
        out.clear();
        let mut points = std::mem::take(&mut self.points);
        let mut letters = word
            .char_indices()
            .filter(|(_, c)| !c.is_alphabetic())
            .map(|(at, c)| (at, at + c.len_utf8()))
            .chain(std::iter::once((word.len(), word.len())));
        let mut start = 0;
        for (end, next) in &mut letters {
            if end > start {
                hyphenator.hyphenate_into(&word[start..end], &mut points);
                out.extend(points.iter().map(|p| start + p));
            }
            start = next;
        }
        self.points = points;
    }
}

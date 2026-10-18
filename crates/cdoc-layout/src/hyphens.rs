//! Hyphenation for the layout: the pattern sets a session has compiled (each once, the first
//! time a paragraph in that language is hyphenated), and the hyphenation points of a word.

use crate::LayoutError;
use cdoc_para::{Hyphenator, Language};

/// Compiled pattern sets, by language.
#[derive(Debug, Clone)]
pub struct Hyphenators {
    compiled: Vec<(Language, Hyphenator)>,
    points: Vec<usize>,
}

impl Hyphenators {
    /// None compiled yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            compiled: Vec::new(),
            points: Vec::new(),
        }
    }

    fn get(&mut self, language: Language) -> Result<&Hyphenator, LayoutError> {
        let index = if let Some(i) = self.compiled.iter().position(|(l, _)| *l == language) {
            i
        } else {
            self.compiled
                .push((language, Hyphenator::for_language(language)?));
            self.compiled.len() - 1
        };
        Ok(&self.compiled[index].1)
    }

    /// The byte offsets in `word` before which the patterns allow a hyphen, ascending. Only
    /// runs of letters are hyphenated (each on its own, as TeX takes a word to end at the
    /// first character that is not a letter), so `word,` and `(word)` hyphenate as `word`.
    pub fn points(
        &mut self,
        language: Language,
        word: &str,
        out: &mut Vec<usize>,
    ) -> Result<(), LayoutError> {
        out.clear();
        let mut points = std::mem::take(&mut self.points);
        let hyphenator = self.get(language)?;
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
        Ok(())
    }
}

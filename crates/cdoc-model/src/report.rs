//! The loss report: what a conversion to or from another format could not carry. Every
//! importer and exporter returns one, so a user (and a test) can see exactly what a lossy
//! format cost. Nothing is dropped without an entry.

use serde::Serialize;

/// One kind of loss, counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Loss {
    /// Where it occurred: a package part, a section of the input, or a model area.
    pub part: String,
    /// What was lost: an element name, or a sentence.
    pub what: String,
    /// How many times.
    pub count: u32,
}

/// Everything lost in one conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Distinct losses in first-seen order.
    pub losses: Vec<Loss>,
}

impl Report {
    /// A report with no losses yet.
    #[must_use]
    pub const fn empty() -> Self {
        Self { losses: Vec::new() }
    }

    /// Whether nothing was lost.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.losses.is_empty()
    }

    /// Record one occurrence.
    pub fn note(&mut self, part: &str, what: &str) {
        self.note_n(part, what, 1);
    }

    /// Record several occurrences.
    pub fn note_n(&mut self, part: &str, what: &str, count: u32) {
        if count == 0 {
            return;
        }
        match self
            .losses
            .iter_mut()
            .find(|l| l.part == part && l.what == what)
        {
            Some(loss) => loss.count += count,
            None => self.losses.push(Loss {
                part: part.to_owned(),
                what: what.to_owned(),
                count,
            }),
        }
    }

    /// Add another report's entries to this one.
    pub fn absorb(&mut self, other: Self) {
        for loss in other.losses {
            self.note_n(&loss.part, &loss.what, loss.count);
        }
    }

    /// Total occurrences with this `what`, over all parts.
    #[must_use]
    pub fn count_of(&self, what: &str) -> u32 {
        self.losses
            .iter()
            .filter(|l| l.what == what)
            .map(|l| l.count)
            .sum()
    }
}

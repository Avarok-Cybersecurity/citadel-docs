//! The loss report: what a conversion to or from another format could not carry. Every
//! importer and exporter returns one, so a user (and a test) can see exactly what a lossy
//! format cost. Nothing is dropped without an entry.

use serde::Serialize;

/// One kind of loss. Serialised untagged, so a counted loss is `{"part", "what", "count"}` and
/// refused macros are `{"part", "bytes"}`: the facade's JSON tells them apart by their fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Loss {
    /// Something the model or the format could not hold, counted.
    Counted {
        /// Where it occurred: a package part, a section of the input, or a model area.
        part: String,
        /// What was lost: an element name, or a sentence.
        what: String,
        /// How many times.
        count: u32,
    },
    /// Macros (a VBA project or its data) were refused: never read, never carried into the
    /// model, never written. The user should be told; the document itself is unaffected.
    MacrosRemoved {
        /// The package part that held them.
        part: String,
        /// Its size.
        bytes: u64,
    },
}

impl Loss {
    /// The part the loss occurred in.
    #[must_use]
    pub fn part(&self) -> &str {
        match self {
            Self::Counted { part, .. } | Self::MacrosRemoved { part, .. } => part,
        }
    }

    /// What was lost, for a counted loss.
    #[must_use]
    pub fn what(&self) -> Option<&str> {
        match self {
            Self::Counted { what, .. } => Some(what),
            Self::MacrosRemoved { .. } => None,
        }
    }
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
        let found = self.losses.iter_mut().find_map(|l| match l {
            Loss::Counted {
                part: p,
                what: w,
                count: c,
            } if p == part && w == what => Some(c),
            _ => None,
        });
        match found {
            Some(c) => *c += count,
            None => self.losses.push(Loss::Counted {
                part: part.to_owned(),
                what: what.to_owned(),
                count,
            }),
        }
    }

    /// Record that the macros in `part` (`bytes` long) were refused.
    pub fn macros_removed(&mut self, part: &str, bytes: u64) {
        self.losses.push(Loss::MacrosRemoved {
            part: part.to_owned(),
            bytes,
        });
    }

    /// Add another report's entries to this one.
    pub fn absorb(&mut self, other: Self) {
        for loss in other.losses {
            match loss {
                Loss::Counted { part, what, count } => self.note_n(&part, &what, count),
                removed @ Loss::MacrosRemoved { .. } => self.losses.push(removed),
            }
        }
    }

    /// Total occurrences with this `what`, over all parts.
    #[must_use]
    pub fn count_of(&self, what: &str) -> u32 {
        self.losses
            .iter()
            .filter_map(|l| match l {
                Loss::Counted { what: w, count, .. } if w == what => Some(*count),
                _ => None,
            })
            .sum()
    }

    /// Whether some counted loss in a part satisfying `part` has a `what` satisfying `what`.
    #[must_use]
    pub fn has(&self, part: impl Fn(&str) -> bool, what: impl Fn(&str) -> bool) -> bool {
        self.losses
            .iter()
            .any(|l| part(l.part()) && l.what().is_some_and(&what))
    }

    /// The macro parts refused, with their sizes.
    #[must_use]
    pub fn macros(&self) -> Vec<(&str, u64)> {
        self.losses
            .iter()
            .filter_map(|l| match l {
                Loss::MacrosRemoved { part, bytes } => Some((part.as_str(), *bytes)),
                Loss::Counted { .. } => None,
            })
            .collect()
    }
}

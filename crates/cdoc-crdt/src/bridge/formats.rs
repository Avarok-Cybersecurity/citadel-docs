//! The distinct formats of a range, for "is this property the same across the selection": the
//! same walk as [`slice`](super::slice), but no text is copied and a format seen before is
//! neither cloned nor kept, so a long range costs one walk plus its distinct formats.

use super::slice::{visit, Formatting, Visit};
use super::{Atom, BridgeError};
use crate::doc::Doc;
use cdoc_model::{Inline, ParagraphProperties, RunProperties};
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::Range;
use std::rc::Rc;

/// The distinct formats of a range, each in order of first appearance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formats {
    /// The direct properties of the paragraphs, the one the range starts in first.
    pub paragraphs: Vec<ParagraphProperties>,
    /// The text's (paragraph, direct character properties) pairs; the paragraph indexes
    /// `paragraphs`.
    pub runs: Vec<(usize, RunProperties)>,
}

struct Scan {
    out: Formats,
    hasher: RandomState,
    paragraphs: HashMap<u64, Vec<usize>>,
    runs: HashMap<u64, Vec<usize>>,
    paragraph: usize,
    last_paragraph: Option<Rc<Atom>>,
    last_run: Option<usize>,
}

impl Scan {
    fn paragraph(&mut self, p: ParagraphProperties) -> usize {
        let bucket = self.paragraphs.entry(self.hasher.hash_one(&p)).or_default();
        if let Some(&i) = bucket.iter().find(|&&i| self.out.paragraphs[i] == p) {
            return i;
        }
        bucket.push(self.out.paragraphs.len());
        self.out.paragraphs.push(p);
        self.out.paragraphs.len() - 1
    }
}

impl Visit for Scan {
    fn text(&mut self, _: &str, mut formatting: Formatting<'_, '_>) -> Result<(), BridgeError> {
        let run = formatting.run()?;
        let same = |r: &(usize, RunProperties)| r.0 == self.paragraph && r.1 == *run;
        if self.last_run.is_some_and(|i| same(&self.out.runs[i])) {
            return Ok(());
        }
        let bucket = self
            .runs
            .entry(self.hasher.hash_one((self.paragraph, run)))
            .or_default();
        let i = if let Some(&i) = bucket.iter().find(|&&i| same(&self.out.runs[i])) {
            i
        } else {
            bucket.push(self.out.runs.len());
            self.out.runs.push((self.paragraph, run.clone()));
            self.out.runs.len() - 1
        };
        self.last_run = Some(i);
        Ok(())
    }

    fn reads_inlines(&self) -> bool {
        false
    }

    fn inline(&mut self, _: Inline) {}

    fn structural(&mut self, atom: &Rc<Atom>) {
        let Atom::Paragraph(p) = &**atom else {
            return;
        };
        if self
            .last_paragraph
            .as_ref()
            .is_some_and(|l| Rc::ptr_eq(l, atom))
        {
            return;
        }
        self.last_paragraph = Some(Rc::clone(atom));
        if self.out.paragraphs[self.paragraph] != *p {
            self.paragraph = self.paragraph(p.clone());
        }
    }
}

/// The distinct formats of the visible units `range`; `first` is the direct properties of the
/// paragraph `range` starts in.
pub fn formats(
    doc: &Doc,
    range: Range<usize>,
    first: ParagraphProperties,
) -> Result<Formats, BridgeError> {
    let mut scan = Scan {
        out: Formats {
            paragraphs: Vec::new(),
            runs: Vec::new(),
        },
        hasher: RandomState::new(),
        paragraphs: HashMap::new(),
        runs: HashMap::new(),
        paragraph: 0,
        last_paragraph: None,
        last_run: None,
    };
    scan.paragraph(first);
    visit(doc, Some(range.start as u64..range.end as u64), &mut scan)?;
    Ok(scan.out)
}

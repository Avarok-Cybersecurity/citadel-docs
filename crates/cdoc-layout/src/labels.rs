//! List labels: `1.`, `1.1`, `iv)`, `•`, computed from the model's numbering and never stored.
//!
//! One implementation, used by the layout and exported to the UI through `cdoc-wasm`, so the
//! editor never re-implements `%1.%2`.
//!
//! Counting follows Word: each numbering instance counts on its own; a paragraph at level `n`
//! advances level `n` and restarts every deeper level; a level that has not been used yet
//! starts at its `start` (or the instance's override for that level).

use cdoc_model::{
    Block, Document, Level, NumId, NumberFormat, Numbering, NumberingRef, Paragraph, StyleError,
};
use std::collections::BTreeMap;

/// A computed label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The label text.
    pub text: String,
    /// The level definition it came from (for its indent, alignment and run formatting).
    pub level: Level,
}

/// Counters for every numbering instance, advanced paragraph by paragraph in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counters<'a> {
    numbering: &'a Numbering,
    counts: BTreeMap<NumId, [Option<u32>; 9]>,
}

impl<'a> Counters<'a> {
    /// No paragraph counted yet.
    #[must_use]
    pub const fn new(numbering: &'a Numbering) -> Self {
        Self {
            numbering,
            counts: BTreeMap::new(),
        }
    }

    fn start(&self, num_id: NumId, level: u8, def: &Level) -> u32 {
        self.numbering
            .instances
            .iter()
            .find(|i| i.id == num_id)
            .and_then(|i| i.overrides.iter().find(|o| o.level == level))
            .map_or(def.start, |o| o.start)
    }

    /// Counts one list paragraph and returns its label, or `None` when the reference names a
    /// numbering or level that does not exist (validation reports those).
    pub fn next(&mut self, reference: NumberingRef) -> Option<Label> {
        let definition = self.numbering.definition_for(reference.num_id)?;
        let level_index = usize::from(reference.level);
        let level = definition.levels.get(level_index)?;
        let first = self.start(reference.num_id, reference.level, level);
        let counts = self.counts.entry(reference.num_id).or_insert([None; 9]);
        counts[level_index] = Some(counts[level_index].map_or(first, |n| n + 1));
        for deeper in counts.iter_mut().skip(level_index + 1) {
            *deeper = None;
        }
        let counts = *counts;
        let text = expand(&level.text, |n| {
            let at = definition.levels.get(n)?;
            let value = counts[n].unwrap_or_else(|| self.start(reference.num_id, level_u8(n), at));
            Some(format_number(value, at.format))
        });
        Some(Label {
            text: if level.format == NumberFormat::Bullet {
                level.text.clone()
            } else {
                text
            },
            level: level.clone(),
        })
    }

    /// The label for a paragraph, after its style cascade: numbering can come from a style.
    pub fn for_paragraph(
        &mut self,
        doc: &Document,
        paragraph: &Paragraph,
    ) -> Result<Option<Label>, StyleError> {
        let resolved = doc.styles.resolve_paragraph(&paragraph.properties)?;
        Ok(resolved.numbering.and_then(|r| self.next(r)))
    }
}

fn level_u8(n: usize) -> u8 {
    u8::try_from(n).unwrap_or(u8::MAX)
}

/// `%1`..`%9` replaced by `number(level - 1)`; anything else copied.
fn expand(template: &str, number: impl Fn(usize) -> Option<String>) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        let digit = chars.peek().and_then(|d| d.to_digit(10));
        match (ch, digit) {
            ('%', Some(d @ 1..=9)) => {
                chars.next();
                out.push_str(&number(d as usize - 1).unwrap_or_default());
            }
            _ => out.push(ch),
        }
    }
    out
}

/// A number written in a list format.
#[must_use]
pub fn format_number(value: u32, format: NumberFormat) -> String {
    match format {
        NumberFormat::Decimal => value.to_string(),
        NumberFormat::LowerLetter => letters(value).to_lowercase(),
        NumberFormat::UpperLetter => letters(value),
        NumberFormat::LowerRoman => roman(value).to_lowercase(),
        NumberFormat::UpperRoman => roman(value),
        NumberFormat::Bullet | NumberFormat::None => String::new(),
    }
}

/// Word's letter numbering: A..Z, then AA..ZZ, then AAA.., the letter repeated.
fn letters(value: u32) -> String {
    if value == 0 {
        return String::new();
    }
    let index = (value - 1) % 26;
    let repeat = (value - 1) / 26 + 1;
    let letter = char::from(b'A' + u8::try_from(index).unwrap_or(0));
    std::iter::repeat_n(letter, repeat as usize).collect()
}

fn roman(mut value: u32) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (n, s) in TABLE {
        while value >= n {
            out.push_str(s);
            value -= n;
        }
    }
    out
}

fn walk_blocks(
    doc: &Document,
    blocks: &[Block],
    counters: &mut Counters<'_>,
    out: &mut Vec<Option<String>>,
) -> Result<(), StyleError> {
    for block in blocks {
        for paragraph in block.paragraphs() {
            out.push(counters.for_paragraph(doc, paragraph)?.map(|l| l.text));
        }
    }
    Ok(())
}

/// The label of every paragraph, in `ProseMirror` document order: per section its headers
/// (default, first, even), then its footers, then its body; then the footnotes. `None` for a
/// paragraph that is not in a list.
///
/// The body counts as one continuous sequence across sections, as Word does; each header,
/// footer and footnote counts on its own, since they are not part of the body's lists.
pub fn list_labels(doc: &Document) -> Result<Vec<Option<String>>, StyleError> {
    let mut out = Vec::new();
    let mut body = Counters::new(&doc.numbering);
    for section in &doc.sections {
        let props = &section.properties;
        for (_, hf) in props
            .headers
            .present()
            .into_iter()
            .chain(props.footers.present())
        {
            walk_blocks(
                doc,
                &hf.blocks,
                &mut Counters::new(&doc.numbering),
                &mut out,
            )?;
        }
        walk_blocks(doc, &section.blocks, &mut body, &mut out)?;
    }
    for note in &doc.footnotes {
        walk_blocks(
            doc,
            &note.blocks,
            &mut Counters::new(&doc.numbering),
            &mut out,
        )?;
    }
    Ok(out)
}

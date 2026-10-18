//! The benchmark corpus: deterministic documents of 1, 10, 100 and 500 pages of prose,
//! headings, lists and tables. Purely procedural from [`SEED`] through [`SplitMix64`]; nothing
//! comes from the environment, so every machine generates the same bytes (the golden test
//! pins the 10-page fingerprint).
//!
//! A document is a run of *units*: a heading, three prose paragraphs, every other unit a list
//! (bullets and numbers alternating), every third unit a table. [`UNITS_PER_PAGE`] is calibrated
//! against the layout engine so `generate(n)` lays out to about `n` pages.

mod blocks;
mod words;

use crate::prng::{fnv1a, SplitMix64};
use cdoc_model::{Block, Document};

pub use blocks::{BULLETS, NUMBERED};

/// The corpus seed.
pub const SEED: u64 = 0x00C1_7ADE_1D0C_2026;

/// The page counts every page-scaled metric runs at.
pub const PAGE_COUNTS: [u32; 4] = [1, 10, 100, 500];

/// The page counts whose fixtures are committed under `crates/cdoc-bench/corpus/`; the larger
/// ones are generated at bench time.
pub const COMMITTED_PAGE_COUNTS: [u32; 2] = [1, 10];

/// Units per page as a ratio (numerator, denominator), calibrated on the Carlito faces.
pub const UNITS_PER_PAGE: (u32, u32) = (15, 11);

/// How many units a document of `pages` pages has (at least one).
#[must_use]
pub fn units_for(pages: u32) -> usize {
    let (num, den) = UNITS_PER_PAGE;
    let units = (u64::from(pages) * u64::from(num)).div_ceil(u64::from(den));
    usize::try_from(units).unwrap_or(usize::MAX).max(1)
}

/// The corpus document of about `pages` pages, from [`SEED`].
#[must_use]
pub fn generate(pages: u32) -> Document {
    generate_seeded(pages, SEED)
}

/// The corpus document of about `pages` pages from `seed`.
#[must_use]
pub fn generate_seeded(pages: u32, seed: u64) -> Document {
    let mut rng = SplitMix64::new(seed);
    let mut doc = Document::blank();
    doc.meta.title = Some(format!("Citadel benchmark corpus, {pages} pages"));
    doc.numbering = blocks::numbering();
    let mut body: Vec<Block> = Vec::new();
    for unit in 0..units_for(pages) {
        body.push(blocks::heading(&mut rng, if unit % 8 == 0 { 1 } else { 2 }));
        for _ in 0..3 {
            body.push(blocks::prose(&mut rng));
        }
        if unit % 2 == 0 {
            let num_id = if unit % 4 == 0 { BULLETS } else { NUMBERED };
            body.extend(blocks::list(&mut rng, num_id));
        }
        if unit % 3 == 2 {
            body.push(blocks::table(&mut rng));
        }
    }
    doc.sections[0].blocks = body;
    doc
}

/// About `chars` characters of corpus prose (whole sentences, cut at `chars`), for the shaping
/// metric. Seeded apart from the documents so their texts do not overlap.
#[must_use]
pub fn latin_text(chars: usize) -> String {
    let mut rng = SplitMix64::new(SEED ^ 0x5348_4150_4521);
    let mut text = String::with_capacity(chars + 200);
    while text.chars().count() < chars {
        text.push_str(&words::sentence(&mut rng));
        text.push(' ');
    }
    text.chars().take(chars).collect()
}

/// `count` words of corpus prose with their punctuation stripped, for the line-breaking metric.
#[must_use]
pub fn word_list(count: usize) -> Vec<String> {
    let mut rng = SplitMix64::new(SEED ^ 0x0050_4152_4121);
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let sentence = words::sentence(&mut rng);
        out.extend(
            sentence
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase()),
        );
    }
    out.truncate(count);
    out
}

/// The model as the JSON the fixtures hold (compact, serde's field order).
pub fn to_json(doc: &Document) -> Result<String, serde_json::Error> {
    serde_json::to_string(doc)
}

/// The corpus fingerprint: FNV-1a of the document's fixture JSON.
pub fn fingerprint(doc: &Document) -> Result<u64, serde_json::Error> {
    Ok(fnv1a(to_json(doc)?.as_bytes()))
}

/// The block a one-character edit goes into: the first prose paragraph at or after the middle
/// of the first section.
#[must_use]
pub fn keystroke_target(doc: &Document) -> Option<usize> {
    let blocks = &doc.sections.first()?.blocks;
    (blocks.len() / 2..blocks.len()).find(|&i| is_body(&blocks[i]))
}

fn is_body(block: &Block) -> bool {
    let Block::Paragraph(p) = block else {
        return false;
    };
    p.properties.style.as_ref().map(cdoc_model::StyleId::as_str) == Some("Normal")
}

/// The document with `typed` appended to the last text run of block `index` of section 0;
/// `None` when that block is not a paragraph with text.
#[must_use]
pub fn typed(doc: &Document, index: usize, typed: &str) -> Option<Document> {
    let mut doc = doc.clone();
    let Block::Paragraph(p) = doc.sections.first_mut()?.blocks.get_mut(index)? else {
        return None;
    };
    let run = p
        .inlines
        .iter_mut()
        .rev()
        .find_map(|i| match &mut i.content {
            cdoc_model::InlineContent::Text(run) => Some(run),
            _ => None,
        })?;
    run.text.push_str(typed);
    Some(doc)
}

/// The fewest trailing words [`keystroke_phrase`] offers to a product's Find.
pub const MIN_PHRASE_WORDS: usize = 4;

fn plain(p: &cdoc_model::Paragraph) -> String {
    p.inlines
        .iter()
        .filter_map(|i| match &i.content {
            cdoc_model::InlineContent::Text(run) => Some(run.text.as_str()),
            _ => None,
        })
        .collect()
}

/// The words a product's own Find lands on to put the caret in [`keystroke_target`]: the
/// paragraph's last words, as few as occur once in the document (case-insensitively, as Find
/// matches). The browser protocol finds them and steps right, leaving the caret at the
/// paragraph's end, where ours types. `None` when no tail of the paragraph is unique.
#[must_use]
pub fn keystroke_phrase(doc: &Document) -> Option<String> {
    let blocks = &doc.sections.first()?.blocks;
    let Block::Paragraph(target) = blocks.get(keystroke_target(doc)?)? else {
        return None;
    };
    let target = plain(target);
    let haystack: Vec<String> = blocks
        .iter()
        .flat_map(Block::paragraphs)
        .map(|p| plain(p).to_lowercase())
        .collect();
    let words: Vec<&str> = target.split_whitespace().collect();
    (MIN_PHRASE_WORDS..=words.len())
        .map(|n| words[words.len() - n..].join(" "))
        .find(|phrase| {
            let needle = phrase.to_lowercase();
            haystack
                .iter()
                .map(|h| h.matches(&needle).count())
                .sum::<usize>()
                == 1
        })
}

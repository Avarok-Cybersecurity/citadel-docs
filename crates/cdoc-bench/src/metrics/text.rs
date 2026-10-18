//! Text: shaping 10 000 Latin characters on Carlito, and Knuth-Plass breaking a 2 000-word
//! hyphenated paragraph. The inputs come from the corpus generator; the competitor runner
//! shapes the same text with the same features.

use super::{BenchError, Ctx, Results, PARA_BREAK, PARA_WORDS, SHAPE_CHARS, SHAPE_LATIN};
use crate::corpus::{latin_text, word_list};
use crate::schema::Measurement;
use cdoc_layout::test_support::FACES;
use cdoc_para::{end_paragraph, Breaker, HyphenKind, Hyphenator, Item, KnuthPlass, Language};
use cdoc_para::{LineWidths, Params};
use cdoc_shape::{Direction, Face, Features, ShapeParams, ShapedRun, Shaper};
use std::hint::black_box;

/// Carlito Regular, the face the shaping metric shapes on (ours and the competitor's).
pub const SHAPE_FACE: &[u8] = FACES[0].2;

/// What the shaping metric shapes with: Latin, the default language, left to right, Word's
/// default features.
#[must_use]
pub fn shape_params() -> ShapeParams {
    ShapeParams {
        script: cdoc_shape::Script::Latin,
        language: cdoc_shape::Language::default_system(),
        direction: Direction::LeftToRight,
        features: Features::word_default(),
    }
}

/// A letter's width, hundredths of a point.
const LETTER: i32 = 500;
/// The measure the paragraph is broken to: 400 pt.
const MEASURE: i32 = 40_000;

fn paragraph(words: &[String], h: &Hyphenator) -> Vec<Item<usize>> {
    let width = |s: &str| i32::try_from(s.chars().count()).unwrap_or(i32::MAX) * LETTER;
    let mut items = Vec::new();
    let mut breaks = Vec::new();
    for (k, word) in words.iter().enumerate() {
        if k > 0 {
            items.push(Item::glue(333, 167, 111));
        }
        h.hyphenate_into(word, &mut breaks);
        let mut last = 0;
        for &b in &breaks {
            items.push(Item::boxed(width(&word[last..b]), k));
            items.push(Item::Hyphen {
                width: 333,
                kind: HyphenKind::Pattern,
            });
            last = b;
        }
        items.push(Item::boxed(width(&word[last..]), k));
    }
    end_paragraph(&mut items);
    items
}

pub(super) fn run(ctx: &Ctx<'_>, out: &mut Results) -> Result<(), BenchError> {
    let text = latin_text(SHAPE_CHARS);
    let params = shape_params();
    let face = Face::parse(SHAPE_FACE, 0)?;
    let mut shaper = Shaper::new(&face);
    let mut run = ShapedRun::empty();
    let t = ctx.time(SHAPE_LATIN, || {
        shaper.shape_into(black_box(&text), &params, &mut run)?;
        black_box(run.glyphs.len());
        Ok(())
    })?;
    out.insert(SHAPE_LATIN.to_owned(), Measurement::Time(t));

    let h = Hyphenator::for_language(Language::EnUs)?;
    let items = paragraph(&word_list(PARA_WORDS), &h);
    let widths = LineWidths::uniform(MEASURE)?;
    let params = Params {
        pretolerance: -1,
        ..Params::tex_plain()
    };
    let t = ctx.time(PARA_BREAK, || {
        black_box(KnuthPlass.break_lines(black_box(&items), &widths, &params)?);
        Ok(())
    })?;
    out.insert(PARA_BREAK.to_owned(), Measurement::Time(t));
    Ok(())
}

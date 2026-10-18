//! The breaker the options name is the one that breaks: on a known paragraph Knuth-Plass and
//! first fit choose different lines, and Knuth-Plass's are the better set by TeX's own
//! measure; a justified line's spaces are `cdoc-para`'s exact justification, so the line ends
//! on the margin to the hundredth of a point.

mod support;

use cdoc_layout::{layout, BreakerChoice, FontPort, Fonts, GlyphRun, Layout, LayoutOptions};
use cdoc_model::{Alignment, Block, Indent, Length};
use cdoc_para::{badness, Params};
use cdoc_shape::Features;

const PROSE: &str = "In olden times when wishing still helped one, there lived a king whose \
    daughters were all beautiful; and the youngest was so beautiful that the sun itself, \
    which has seen so much, was astonished whenever it shone in her face. Close by the \
    king's castle lay a great dark forest, and under an old lime-tree in the forest was a \
    well, and when the day was very warm, the king's child went out into the forest and sat \
    down by the side of the cool fountain.";

/// A4 Normal's text area with `inset` taken off both sides.
const LEFT: i32 = 7200;
const RIGHT: i32 = 59530 - 7200;

fn paragraph(alignment: Alignment, inset: i32) -> cdoc_model::Document {
    let mut block = support::para(PROSE);
    if let Block::Paragraph(p) = &mut block {
        p.properties.alignment = Some(alignment);
        p.properties.indent = Some(Indent {
            left: Some(Length(inset)),
            right: Some(Length(inset)),
            first_line: None,
        });
    }
    support::doc_with(vec![block])
}

fn with(breaker: BreakerChoice, doc: &cdoc_model::Document) -> Layout {
    layout(doc, &support::fonts(), &LayoutOptions::word_like(breaker)).expect("lays out")
}

/// The width of a space in `run`'s face and size, as the layout measures one.
fn space(run: &GlyphRun, layout: &Layout) -> i32 {
    let lib = support::fonts();
    let mut fonts = Fonts::new(&lib as &dyn FontPort);
    let key = &layout.faces[usize::from(run.face.0)];
    let id = fonts
        .resolve(&key.family, key.bold, key.italic)
        .expect("face");
    fonts.width(id, run.size, " ", Features::word_default())
}

/// TeX's demerits for the ragged lines (all but the last): `(line penalty + badness)²`, the
/// badness of stretching each line's spaces (half a space each, as the layout's glue) to the
/// measure.
fn demerits(layout: &Layout, measure: i32) -> i64 {
    let params = Params::word_like();
    let runs: Vec<&GlyphRun> = layout.pages[0].runs().collect();
    runs[..runs.len() - 1]
        .iter()
        .map(|run| {
            let spaces = i64::try_from(run.text.trim_end().matches(' ').count()).expect("count");
            let stretch = spaces * i64::from(space(run, layout) / 2);
            let short = i64::from(measure - run.width());
            let base = i64::from(params.line_penalty) + badness(short, stretch);
            base * base
        })
        .sum()
}

#[test]
fn knuth_plass_and_first_fit_differ_and_knuth_plass_has_lower_demerits() {
    let inset = 11000;
    let doc = paragraph(Alignment::Left, inset);
    let kp = with(BreakerChoice::KnuthPlass(Params::word_like()), &doc);
    let ff = with(BreakerChoice::FirstFit, &doc);
    let text = |l: &Layout| {
        l.pages[0]
            .runs()
            .map(|r| r.text.clone())
            .collect::<Vec<_>>()
    };
    assert_ne!(text(&kp), text(&ff), "the breakers agree on this paragraph");
    let measure = RIGHT - LEFT - 2 * inset;
    let (k, f) = (demerits(&kp, measure), demerits(&ff, measure));
    assert!(k < f, "Knuth-Plass {k} vs first fit {f}");
}

#[test]
fn justified_lines_end_exactly_on_the_margin() {
    let inset = 9000;
    let layout = support::laid_out(&paragraph(Alignment::Justify, inset));
    let runs: Vec<&GlyphRun> = layout.pages[0].runs().collect();
    assert!(runs.len() >= 4, "{runs:?}");
    for run in &runs[..runs.len() - 1] {
        assert_eq!(run.x, LEFT + inset);
        assert_eq!(run.x + run.width(), RIGHT - inset, "{:?}", run.text);
    }
    let last = runs.last().expect("runs");
    assert!(
        last.x + last.width() < RIGHT - inset,
        "the last line is ragged"
    );
}

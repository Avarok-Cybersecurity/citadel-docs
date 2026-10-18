//! The frozen control fixture: the kitchen sink followed by ragged paragraphs that exercise
//! every input the line breaker sees (indents and hangs, mixed styles and sizes, small and all
//! capitals, superscripts, tabs, forced line breaks, runs of spaces, words wider than a column,
//! kerning pairs and ligature letters). Its layout before shaping and Knuth-Plass were
//! integrated is committed as `golden/frozen_greedy.txt`; with every feature off and first-fit
//! breaking the layout must still produce it.

#![allow(dead_code, reason = "each target uses a different part")]

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{
    Alignment, Block, Document, FirstLine, Indent, Inline, Length, Paragraph, ParagraphProperties,
    RunProperties, VerticalAlign,
};

const PROSE: &str = "AVA To Wa fly office flourish affinity, Yes. We typeset every line \
    by measuring each word in the face the styles resolve and breaking where the measure \
    runs out, so the editor shows what the file prints.";

fn run(text: &str, props: RunProperties) -> Inline {
    Inline::text(text, props)
}

fn plain(text: &str) -> Inline {
    run(text, RunProperties::none())
}

fn paragraph(alignment: Alignment, indent: Option<Indent>, inlines: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style: Some(cdoc_model::builtin::ids::normal()),
            alignment: Some(alignment),
            indent,
            ..ParagraphProperties::none()
        },
        inlines,
    })
}

fn indent(left: i32, right: i32, first: Option<FirstLine>) -> Indent {
    Indent {
        left: Some(Length(left)),
        right: Some(Length(right)),
        first_line: first,
    }
}

fn styled() -> Vec<Inline> {
    let bold = RunProperties {
        bold: Some(true),
        ..RunProperties::none()
    };
    let italic = RunProperties {
        italic: Some(true),
        ..RunProperties::none()
    };
    let big = RunProperties {
        size: Some(cdoc_model::FontSize(30)),
        ..RunProperties::none()
    };
    let caps = RunProperties {
        small_caps: Some(true),
        ..RunProperties::none()
    };
    let upper = RunProperties {
        all_caps: Some(true),
        ..RunProperties::none()
    };
    let sup = RunProperties {
        vertical_align: Some(VerticalAlign::Superscript),
        ..RunProperties::none()
    };
    vec![
        plain("Mixed "),
        run("bold office ", bold),
        run("italic affluent ", italic.clone()),
        run("LARGE Type ", big),
        run("small capitals Wave ", caps),
        run("all caps fly ", upper),
        plain("E = mc"),
        run("2", sup),
        plain(" then"),
        run("half", italic),
        plain("word joined across runs and more prose to wrap. "),
        plain(PROSE),
    ]
}

/// The fixture document.
#[must_use]
pub fn document() -> Document {
    let mut doc = kitchen_sink();
    let long_word = "https://example.org/a/very/long/path/that/cannot/fit/on/any/line/at/all/\
        because/it/keeps/going/and/going/well/past/the/measure/of/a/column";
    let blocks = vec![
        paragraph(Alignment::Left, None, vec![plain(PROSE)]),
        paragraph(
            Alignment::Left,
            Some(indent(720, 0, Some(FirstLine::Indent { by: Length(1440) }))),
            vec![plain(PROSE), plain(" "), plain(PROSE)],
        ),
        paragraph(
            Alignment::Left,
            Some(indent(
                1800,
                360,
                Some(FirstLine::Hanging { by: Length(1080) }),
            )),
            vec![plain(PROSE)],
        ),
        paragraph(Alignment::Center, None, vec![plain(PROSE)]),
        paragraph(
            Alignment::Right,
            Some(indent(0, 1440, None)),
            vec![plain(PROSE)],
        ),
        paragraph(Alignment::Left, None, styled()),
        paragraph(
            Alignment::Left,
            None,
            vec![plain(
                "Name:\tValue\tMore\nSecond\tline  with  double  spaces\n  lead",
            )],
        ),
        paragraph(
            Alignment::Left,
            None,
            vec![plain(
                "A tab far along a line of words that keeps on going and going\tand then wraps on.",
            )],
        ),
        paragraph(Alignment::Left, None, vec![plain(long_word)]),
        paragraph(
            Alignment::Left,
            None,
            vec![
                plain("Before the long word "),
                plain(long_word),
                plain(" after it."),
            ],
        ),
        paragraph(
            Alignment::Left,
            None,
            vec![plain("   Leading spaces on a first line.")],
        ),
    ];
    doc.sections[0].blocks.extend(blocks);
    doc
}

/// Every drawing on every page, one per line: text runs as `T x y size face text`, the rest
/// by their debug form, then the links. The control's golden form.
#[must_use]
pub fn dump(layout: &cdoc_layout::Layout) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (i, page) in layout.pages.iter().enumerate() {
        let _ = writeln!(out, "page {i} {}x{}", page.width, page.height);
        for run in page.runs() {
            let _ = writeln!(
                out,
                "  T {} {} {} f{} {:?}",
                run.x, run.y, run.size, run.face.0, run.text
            );
        }
        for link in &page.links {
            let _ = writeln!(out, "  L {:?} {}", link.rect, link.href);
        }
    }
    out
}

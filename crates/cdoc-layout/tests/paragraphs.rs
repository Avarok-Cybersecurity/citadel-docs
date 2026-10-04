//! Line breaking, alignment and indents, measured against the faces themselves.

mod support;

use cdoc_layout::{FontPort, Fonts, Layout, TextRun};
use cdoc_model::{Alignment, Block, FirstLine, Indent, Length, Paragraph};

const PROSE: &str = "The quick brown fox jumps over the lazy dog and keeps running across the \
    wide field until the evening light fades behind the distant hills, where it finally rests \
    beside a quiet stream that winds between old stones and tall grass.";

/// Text area of A4 Normal: one-inch margins.
const LEFT: i32 = 7200;
const RIGHT: i32 = 59530 - 7200;

fn styled(text: &str, f: impl FnOnce(&mut Paragraph)) -> Block {
    let mut block = support::para(text);
    if let Block::Paragraph(p) = &mut block {
        f(p);
    }
    block
}

fn runs(layout: &Layout) -> Vec<&TextRun> {
    layout.pages[0].runs().collect()
}

fn end_of(run: &TextRun, layout: &Layout) -> i32 {
    let lib = support::fonts();
    let mut fonts = Fonts::new(&lib as &dyn FontPort);
    let key = &layout.faces[usize::from(run.face.0)];
    let id = fonts
        .resolve(&key.family, key.bold, key.italic)
        .expect("face");
    run.x + fonts.width(id, run.size, &run.text)
}

#[test]
fn left_aligned_lines_fit_the_column_and_fill_it() {
    let layout = support::laid_out(&support::doc_with(vec![support::para(PROSE)]));
    let lines = runs(&layout);
    assert!(lines.len() >= 3, "{lines:?}");
    for run in &lines {
        assert_eq!(run.x, LEFT);
        assert!(end_of(run, &layout) <= RIGHT, "{:?} overruns", run.text);
    }
    let joined: Vec<&str> = lines.iter().map(|r| r.text.trim_end()).collect();
    assert_eq!(
        joined.join(" "),
        PROSE.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    for pair in lines.windows(2) {
        let next = pair[1].text.split(' ').next().unwrap_or("");
        let longer = TextRun {
            text: format!("{} {next}", pair[0].text.trim_end()),
            ..pair[0].clone()
        };
        assert!(
            end_of(&longer, &layout) > RIGHT,
            "{:?} would have fit",
            longer.text
        );
    }
}

#[test]
fn justified_lines_end_on_the_right_margin_except_the_last() {
    let block = styled(PROSE, |p| p.properties.alignment = Some(Alignment::Justify));
    let layout = support::laid_out(&support::doc_with(vec![block]));
    let words = runs(&layout);
    let mut lines: Vec<Vec<&&TextRun>> = Vec::new();
    for run in &words {
        match lines.last_mut() {
            Some(line) if line[0].y == run.y => line.push(run),
            _ => lines.push(vec![run]),
        }
    }
    assert!(lines.len() >= 3);
    for line in &lines[..lines.len() - 1] {
        let end = end_of(line.last().expect("words"), &layout);
        assert!(
            (RIGHT - end).abs() <= 2,
            "line ends at {end}, margin {RIGHT}"
        );
    }
    let last = lines.last().expect("lines");
    assert!(
        end_of(last.last().expect("words"), &layout) < RIGHT - 1000,
        "last line is ragged"
    );
}

#[test]
fn centred_and_right_aligned_lines_balance_and_flush() {
    for (alignment, check) in [(Alignment::Center, 0_i32), (Alignment::Right, 1)] {
        let block = styled("short", |p| p.properties.alignment = Some(alignment));
        let layout = support::laid_out(&support::doc_with(vec![block]));
        let run = runs(&layout)[0].clone();
        let end = end_of(&run, &layout);
        if check == 0 {
            assert!(
                ((run.x - LEFT) - (RIGHT - end)).abs() <= 1,
                "centred: {} / {end}",
                run.x
            );
        } else {
            assert!((RIGHT - end).abs() <= 1, "right: {end}");
        }
    }
}

#[test]
fn first_line_and_hanging_indents_move_only_the_first_line() {
    for (first_line, first_x) in [
        (FirstLine::Indent { by: Length(3600) }, LEFT + 7200 + 3600),
        (FirstLine::Hanging { by: Length(3600) }, LEFT + 7200 - 3600),
    ] {
        let block = styled(PROSE, |p| {
            p.properties.indent = Some(Indent {
                left: Some(Length(7200)),
                right: Some(Length(7200)),
                first_line: Some(first_line),
            });
        });
        let layout = support::laid_out(&support::doc_with(vec![block]));
        let lines = runs(&layout);
        assert_eq!(lines[0].x, first_x);
        assert!(lines[1..].iter().all(|r| r.x == LEFT + 7200));
        assert!(lines.iter().all(|r| end_of(r, &layout) <= RIGHT - 7200));
    }
}

#[test]
fn a_word_wider_than_the_column_is_broken_rather_than_lost() {
    let url = "https://example.com/".to_owned() + &"segment/".repeat(40);
    let layout = support::laid_out(&support::doc_with(vec![support::para(&url)]));
    let lines = runs(&layout);
    assert!(lines.len() > 1);
    assert!(lines.iter().all(|r| end_of(r, &layout) <= RIGHT));
    assert_eq!(
        lines.iter().map(|r| r.text.as_str()).collect::<String>(),
        url
    );
}

#[test]
fn space_before_and_after_separate_paragraphs() {
    let layout = support::laid_out(&support::doc_with(vec![
        support::para("one"),
        support::para("two"),
    ]));
    let r = runs(&layout);
    // Normal: 8 pt after, 1.08 line spacing (259/240) at 11 pt Carlito.
    let gap = r[1].y - r[0].y;
    assert!((2100..2300).contains(&gap), "baseline gap {gap}");
}

#[test]
fn a_word_made_of_differently_styled_runs_is_never_broken_between_them() {
    let bold = cdoc_model::RunProperties {
        bold: Some(true),
        ..cdoc_model::RunProperties::none()
    };
    let mut inlines = Vec::new();
    for i in 0..60 {
        let plain = "ab".repeat(1 + i % 4);
        inlines.push(cdoc_model::Inline::text(
            plain,
            cdoc_model::RunProperties::none(),
        ));
        let strong = "CD".repeat(1 + i % 3) + " ";
        inlines.push(cdoc_model::Inline::text(strong, bold.clone()));
    }
    let block = styled("", |p| p.inlines = inlines);
    let layout = support::laid_out(&support::doc_with(vec![block]));
    let lines = runs(&layout);
    let mut starts = Vec::new();
    for (i, run) in lines.iter().enumerate() {
        if i == 0 || lines[i - 1].y != run.y {
            starts.push(run.text.as_str());
        }
    }
    assert!(starts.len() > 2);
    assert!(starts.iter().all(|t| t.starts_with("ab")), "{starts:?}");
}

//! Plain text to model: paragraphs separated by blank lines.

use crate::options::{SingleNewline, TxtReadOptions};
use cdoc_model::Block;
use cdoc_model::{
    BreakKind, Document, Inline, InlineContent, Paragraph, ParagraphProperties, Report,
    RunProperties, StyleId,
};

const FORM_FEED: char = '\u{c}';

/// The inlines of one line: text, with tabs as tab inlines and form feeds as page breaks.
fn line_inlines(line: &str, out: &mut Vec<Inline>) {
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Inline>| {
        if !text.is_empty() {
            out.push(Inline::text(std::mem::take(text), RunProperties::none()));
        }
    };
    for c in line.chars() {
        match c {
            '\t' => {
                flush(&mut text, out);
                out.push(Inline::plain(InlineContent::Tab));
            }
            FORM_FEED => {
                flush(&mut text, out);
                out.push(Inline::plain(InlineContent::Break {
                    kind: BreakKind::Page,
                }));
            }
            c => text.push(c),
        }
    }
    flush(&mut text, out);
}

/// Read plain text. Line endings (`\r\n`, `\r`) are normalised; a run of blank lines (empty
/// or only whitespace) separates paragraphs; a tab is a tab and a form feed a page break.
/// Nothing is lost, so the report is always empty; it is returned so every converter has the
/// same shape.
#[must_use]
pub fn txt_to_document(text: &str, options: &TxtReadOptions) -> (Document, Report) {
    let normalised = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in normalised.split('\n') {
        if line.trim().is_empty() {
            if paragraphs.last().is_some_and(|p| !p.is_empty()) {
                paragraphs.push(Vec::new());
            }
        } else if let Some(last) = paragraphs.last_mut() {
            last.push(line);
        }
    }
    let blocks: Vec<Block> = paragraphs
        .into_iter()
        .filter(|lines| !lines.is_empty())
        .map(|lines| {
            let mut inlines = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    inlines.push(match options.single_newline {
                        SingleNewline::Break => Inline::plain(InlineContent::Break {
                            kind: BreakKind::Line,
                        }),
                        SingleNewline::Space => Inline::text(" ", RunProperties::none()),
                    });
                }
                line_inlines(line, &mut inlines);
            }
            Block::Paragraph(Paragraph {
                properties: ParagraphProperties {
                    style: Some(StyleId::new("Normal")),
                    ..ParagraphProperties::none()
                },
                inlines,
            })
        })
        .collect();
    let mut doc = Document::blank();
    if !blocks.is_empty() {
        doc.sections[0].blocks = blocks;
    }
    doc.normalize();
    (doc, Report::empty())
}

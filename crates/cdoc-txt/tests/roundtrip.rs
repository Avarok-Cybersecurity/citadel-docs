//! Properties: text read and written back is the text, normalised; a plain document written
//! and read back is the same document.

use cdoc_model::{
    Block, Document, Inline, InlineContent, Paragraph, ParagraphProperties, RunProperties, StyleId,
};
use cdoc_txt::{document_to_txt, txt_to_document, SingleNewline, TxtReadOptions};
use proptest::prelude::*;

/// Test options (PCND): the properties are about line structure, so newlines are breaks.
const BREAK: TxtReadOptions = TxtReadOptions {
    single_newline: SingleNewline::Break,
};

/// A line with at least one visible character, possibly padded, with tabs and non-ASCII.
fn line() -> BoxedStrategy<String> {
    proptest::string::string_regex(
        "[ \t]{0,3}[a-zA-Z0-9é😀.,!?-]{1,8}([ \t]{1,2}[a-zA-Z0-9]{1,5}){0,3}[ \t]{0,2}",
    )
    .expect("regex")
    .boxed()
}

fn paragraphs() -> BoxedStrategy<Vec<Vec<String>>> {
    prop::collection::vec(prop::collection::vec(line(), 1..=3), 1..=4).boxed()
}

fn render(paragraphs: &[Vec<String>], separator: &str) -> String {
    paragraphs
        .iter()
        .map(|p| p.join("\n"))
        .collect::<Vec<_>>()
        .join(separator)
}

fn plain_document(paragraphs: &[Vec<String>]) -> Document {
    let mut doc = Document::blank();
    doc.sections[0].blocks = paragraphs
        .iter()
        .map(|lines| {
            let mut inlines = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    inlines.push(Inline::plain(InlineContent::Break {
                        kind: cdoc_model::BreakKind::Line,
                    }));
                }
                for (j, piece) in line.split('\t').enumerate() {
                    if j > 0 {
                        inlines.push(Inline::plain(InlineContent::Tab));
                    }
                    if !piece.is_empty() {
                        inlines.push(Inline::text(piece, RunProperties::none()));
                    }
                }
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
    doc
}

proptest! {
    #[test]
    fn text_is_normalised_to_one_blank_line_between_paragraphs(
        ps in paragraphs(),
        gaps in prop::collection::vec(prop::sample::select(&["\n\n", "\n \n", "\r\n\r\n", "\n\n\n\n"][..]), 4),
    ) {
        let mut input = String::new();
        for (i, p) in ps.iter().enumerate() {
            if i > 0 { input.push_str(gaps[i - 1]); }
            input.push_str(&p.join("\n"));
        }
        let (doc, _) = txt_to_document(&input, &BREAK);
        prop_assert_eq!(document_to_txt(&doc).0, format!("{}\n", render(&ps, "\n\n")));
    }

    #[test]
    fn a_plain_document_survives_text(ps in paragraphs()) {
        let doc = plain_document(&ps);
        let (text, report) = document_to_txt(&doc);
        prop_assert!(report.is_lossless(), "{:?}", report);
        let (mut back, _) = txt_to_document(&text, &BREAK);
        let mut expected = doc;
        expected.normalize();
        back.normalize();
        prop_assert_eq!(back.sections[0].blocks.clone(), expected.sections[0].blocks.clone());
    }
}

//! Lists, rules, code blocks and footnote definitions.

use super::Reader;
use crate::write::CODE_FONT;
use cdoc_model::{
    Block, Border, BorderStyle, BorderWidth, Borders, Footnote, FootnoteId, Inline, InlineContent,
    Length, Paragraph, ParagraphProperties, RunProperties, StyleId, TextRun, MAX_LEVEL,
};

impl Reader<'_> {
    pub(super) fn start_list(&mut self, start: Option<u64>) {
        self.flush();
        let depth = u8::try_from(self.lists.len())
            .unwrap_or(MAX_LEVEL)
            .min(MAX_LEVEL);
        let num_id = match start {
            None => self.list_builder.bullet_list(),
            Some(n) => self
                .list_builder
                .ordered_list(depth, u32::try_from(n).unwrap_or(u32::MAX)),
        };
        self.lists.push(super::ListState {
            num_id,
            level: depth,
        });
    }

    pub(super) fn rule(&mut self) {
        self.flush();
        let border = Border {
            style: BorderStyle::Single,
            width: BorderWidth(6),
            color: None,
            space: Length::from_points(1),
        };
        self.out.push(Block::Paragraph(Paragraph {
            properties: ParagraphProperties {
                borders: Some(Borders {
                    bottom: Some(border),
                    ..Borders::none()
                }),
                ..ParagraphProperties::none()
            },
            inlines: Vec::new(),
        }));
    }

    pub(super) fn end_code(&mut self) {
        let text = self.code.take().unwrap_or_default();
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let font = RunProperties {
            font: Some(CODE_FONT.to_owned()),
            ..RunProperties::none()
        };
        let mut inlines = Vec::new();
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                inlines.push(Inline::plain(InlineContent::Break {
                    kind: cdoc_model::BreakKind::Line,
                }));
            }
            if !line.is_empty() {
                inlines.push(Inline::plain(InlineContent::Text(TextRun {
                    text: line.to_owned(),
                    properties: font.clone(),
                })));
            }
        }
        self.lose("code block (read as monospace paragraph)");
        self.out.push(Block::Paragraph(Paragraph {
            properties: ParagraphProperties {
                style: Some(StyleId::new("Normal")),
                ..ParagraphProperties::none()
            },
            inlines,
        }));
    }

    /// The id for a footnote label. Numeric labels keep their number; names get ids past any
    /// number a document is likely to use, in order of first appearance.
    pub(super) fn footnote_id(&mut self, label: &str) -> FootnoteId {
        if let Ok(n) = label.parse::<u32>() {
            return FootnoteId(n);
        }
        let at = if let Some(at) = self.named_labels.iter().position(|l| l == label) {
            at
        } else {
            self.named_labels.push(label.to_owned());
            self.named_labels.len() - 1
        };
        FootnoteId(10_000 + u32::try_from(at).unwrap_or(0))
    }

    pub(super) fn end_footnote(&mut self) {
        self.flush();
        let mut blocks = std::mem::take(&mut self.out);
        self.out = self.saved.pop().unwrap_or_default();
        self.in_footnote = false;
        if blocks.is_empty() {
            blocks.push(Block::Paragraph(Paragraph {
                properties: ParagraphProperties::none(),
                inlines: Vec::new(),
            }));
        }
        let label = self.current_footnote.take().unwrap_or_default();
        let id = self.footnote_id(&label);
        self.footnotes.push(Footnote { id, blocks });
    }
}

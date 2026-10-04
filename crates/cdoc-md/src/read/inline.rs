//! Text, breaks, images and footnote marks.

use super::Reader;
use crate::write::CODE_FONT;
use cdoc_model::{
    BreakKind, ContentId, Inline, InlineContent, InlineImage, Marks, RunProperties, StyleId,
    TextRun,
};

fn decode_destination(dest: &str) -> String {
    dest.replace("%3C", "<")
        .replace("%3E", ">")
        .replace("%0A", "\n")
        .replace("%0D", "\r")
}

impl Reader<'_> {
    fn marks(&self) -> Marks {
        Marks {
            link: self.fmt.link.clone(),
            ..Marks::none()
        }
    }

    pub(super) fn text(&mut self, text: &str, code: bool) {
        if let Some(buffer) = &mut self.code {
            buffer.push_str(text);
            return;
        }
        if let Some((_, alt)) = &mut self.fmt.image {
            alt.push_str(text);
            return;
        }
        if text.is_empty() || self.html {
            return;
        }
        self.ensure_paragraph();
        let properties = RunProperties {
            style: self.fmt.link.is_some().then(|| StyleId::new("Hyperlink")),
            font: code.then(|| CODE_FONT.to_owned()),
            bold: (self.fmt.bold > 0).then_some(true),
            italic: (self.fmt.italic > 0).then_some(true),
            strike: (self.fmt.strike > 0).then_some(true),
            ..RunProperties::none()
        };
        self.inlines.push(Inline {
            content: InlineContent::Text(TextRun {
                text: text.to_owned(),
                properties,
            }),
            marks: self.marks(),
        });
    }

    pub(super) fn line_break(&mut self) {
        self.ensure_paragraph();
        self.inlines.push(Inline {
            content: InlineContent::Break {
                kind: BreakKind::Line,
            },
            marks: self.marks(),
        });
    }

    pub(super) fn inline_html(&mut self, html: &str) {
        let tag: String = html
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        match tag.as_str() {
            "<br>" | "<br/>" => self.line_break(),
            "<del>" | "<s>" | "<strike>" => self.fmt.strike += 1,
            "</del>" | "</s>" | "</strike>" => self.fmt.strike = self.fmt.strike.saturating_sub(1),
            "<strong>" | "<b>" => self.fmt.bold += 1,
            "</strong>" | "</b>" => self.fmt.bold = self.fmt.bold.saturating_sub(1),
            "<em>" | "<i>" => self.fmt.italic += 1,
            "</em>" | "</i>" => self.fmt.italic = self.fmt.italic.saturating_sub(1),
            _ => self.lose("inline HTML (dropped)"),
        }
    }

    pub(super) fn footnote_ref(&mut self, label: &str) {
        self.ensure_paragraph();
        let id = self.footnote_id(label);
        self.inlines.push(Inline {
            content: InlineContent::FootnoteRef { id },
            marks: self.marks(),
        });
    }

    pub(super) fn end_image(&mut self) {
        let Some((dest, alt)) = self.fmt.image.take() else {
            return;
        };
        self.ensure_paragraph();
        if let Some(id) = dest.strip_prefix("cid:") {
            self.inlines.push(Inline {
                content: InlineContent::Image(InlineImage {
                    content_id: ContentId::new(decode_destination(id)),
                    width: self.opts.image_width,
                    height: self.opts.image_height,
                    alt: (!alt.is_empty()).then_some(alt),
                }),
                marks: self.marks(),
            });
        } else {
            self.lose("image with a URL, not a content id (its alt text is kept)");
            self.text(&alt, false);
        }
    }
}

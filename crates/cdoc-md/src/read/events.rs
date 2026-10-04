//! Event dispatch: what each tag opens and closes.

use super::table::TableBuild;
use super::{Pending, Reader};
use pulldown_cmark::{Tag, TagEnd};

impl Reader<'_> {
    pub(super) fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.begin(Pending::Paragraph),
            Tag::Heading { level, .. } => self.begin(Pending::Heading(level as u8)),
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote_depth += 1;
                if self.quote_depth > 1 {
                    self.lose("nested block quote (flattened)");
                }
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.flush();
                self.html = true;
                self.lose("HTML block (dropped)");
            }
            Tag::List(start) => self.start_list(start),
            Tag::Item => {
                self.flush();
                self.item_first = true;
            }
            Tag::FootnoteDefinition(label) => {
                self.flush();
                self.saved.push(std::mem::take(&mut self.out));
                self.in_footnote = true;
                self.current_footnote = Some(label.to_string());
            }
            Tag::Table(aligns) => {
                self.flush();
                self.table = Some(TableBuild::new(aligns));
            }
            Tag::TableHead | Tag::TableRow => self.start_row(matches!(tag, Tag::TableHead)),
            Tag::TableCell => self.begin_cell(),
            Tag::Emphasis => self.fmt.italic += 1,
            Tag::Strong => self.fmt.bold += 1,
            Tag::Strikethrough => self.fmt.strike += 1,
            Tag::Link {
                dest_url, title, ..
            } => {
                self.ensure_paragraph();
                self.fmt.link = Some(cdoc_model::Link {
                    href: dest_url.to_string(),
                    title: (!title.is_empty()).then(|| title.to_string()),
                });
            }
            Tag::Image { dest_url, .. } => {
                self.fmt.image = Some((dest_url.to_string(), String::new()));
            }
            _ => self.lose("construct this reader does not model"),
        }
    }

    pub(super) fn end(&mut self, end: TagEnd) {
        match end {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::TableCell => {
                self.flush();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => self.end_code(),
            TagEnd::HtmlBlock => self.html = false,
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::FootnoteDefinition => self.end_footnote(),
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead | TagEnd::TableRow => self.end_row(),
            TagEnd::Emphasis => self.fmt.italic -= 1,
            TagEnd::Strong => self.fmt.bold -= 1,
            TagEnd::Strikethrough => self.fmt.strike -= 1,
            TagEnd::Link => self.fmt.link = None,
            TagEnd::Image => self.end_image(),
            _ => {}
        }
    }
}

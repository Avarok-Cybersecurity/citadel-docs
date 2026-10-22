//! Where a paragraph lands given the quote or list it is in, and the paragraphs a list item
//! needs when it does not start with one.

use crate::blocks::{styled, Ctx, Reader};
use cdoc_model::builtin::{ids, list_indent};
use cdoc_model::{Block, Indent, Paragraph};

impl Reader {
    /// Text found where a block belongs, as a paragraph of its own.
    pub(crate) fn flush_loose(&mut self, ctx: Ctx) {
        if !self.loose.is_empty() {
            let inlines = std::mem::take(&mut self.loose);
            self.place(styled(ids::normal(), inlines), ctx, true);
        }
    }

    /// Add `paragraph`, a plain one or a heading, code block or rule, to the document as `ctx`
    /// shapes it: in a list a plain paragraph is the item's numbered first paragraph or an
    /// unnumbered one at its indent, in a quote it is a Quote.
    pub(crate) fn place(&mut self, mut paragraph: Paragraph, ctx: Ctx, plain: bool) {
        if let Some(list) = ctx.list {
            if plain {
                paragraph.properties.style = Some(ids::list_paragraph());
                match self.number.take() {
                    Some(number) => paragraph.properties.numbering = Some(number),
                    None => {
                        paragraph.properties.indent = Some(Indent {
                            left: Some(list_indent(list.level)),
                            right: None,
                            first_line: None,
                        });
                    }
                }
            } else {
                self.number_pending(true);
                self.lose(
                    "a heading, rule or code block inside a list item (read outside the list)",
                );
            }
        } else if ctx.quote {
            if plain {
                paragraph.properties.style = Some(ids::quote());
            } else {
                self.lose(
                    "a heading, rule, list or code block inside a block quote (the quote not kept)",
                );
            }
        }
        self.out.push(Block::Paragraph(paragraph));
    }

    /// The pending item number on an empty paragraph of its own, because something other
    /// than a paragraph comes first in the item; `added` says whether that is a loss.
    pub(crate) fn number_pending(&mut self, added: bool) {
        if let Some(number) = self.number.take() {
            if added {
                self.lose("a list item without a first paragraph (an empty one added)");
            }
            let mut paragraph = styled(ids::list_paragraph(), Vec::new());
            paragraph.properties.numbering = Some(number);
            self.out.push(Block::Paragraph(paragraph));
        }
    }
}

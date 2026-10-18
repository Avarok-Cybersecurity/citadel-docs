//! Paragraph content: character data collapsed by the ODF white-space rule, spans resolved
//! through their styles, hyperlinks, breaks, tabs, fields, input fields, images, and the marks
//! (comments, changes, notes) other modules read.

use super::ctx::{raw_text, Ctx};
use super::Reader;
use crate::text::spaces;
use cdoc_model::{BreakKind, Field, FieldKind, Inline, InlineContent, Link, RunProperties};
use cdoc_pkg::tag::Tagged;
use cdoc_pkg::xml::{Child, Node};
use cdoc_pkg::PackageError;

impl Reader<'_> {
    /// Read the content of `node` into `out`.
    pub(super) fn inlines(
        &mut self,
        node: &Node,
        ctx: &mut Ctx<'_>,
        out: &mut Vec<Inline>,
    ) -> Result<(), PackageError> {
        for child in &node.children {
            match child {
                Child::Text(t) => {
                    let text = ctx.space.collapse(t);
                    ctx.text(text, out);
                }
                Child::Node(n) => self.element(n, ctx, out)?,
            }
        }
        Ok(())
    }

    fn element(
        &mut self,
        n: &Node,
        ctx: &mut Ctx<'_>,
        out: &mut Vec<Inline>,
    ) -> Result<(), PackageError> {
        match n.name.as_str() {
            "text:s" => {
                ctx.text(spaces(n), out);
                ctx.space.white();
            }
            "text:tab" => {
                ctx.push(InlineContent::Tab, out);
                ctx.space.white();
            }
            "text:line-break" => {
                ctx.push(
                    InlineContent::Break {
                        kind: BreakKind::Line,
                    },
                    out,
                );
                ctx.space.white();
            }
            "text:span" => {
                let span = n
                    .get("text:style-name")
                    .map_or_else(RunProperties::none, |s| self.span_style(s));
                let inner = ctx.run.overlay(&span);
                let saved = std::mem::replace(&mut ctx.run, inner);
                self.inlines(n, ctx, out)?;
                ctx.run = saved;
            }
            "text:a" | "draw:a" => {
                let link = n.get("xlink:href").map(|href| Link {
                    href: href.to_owned(),
                    title: n.get("office:title").map(str::to_owned),
                });
                if link.is_none() {
                    self.log
                        .lose("a hyperlink without a target read as its text");
                }
                let saved = std::mem::replace(&mut ctx.link, link);
                self.inlines(n, ctx, out)?;
                ctx.link = saved;
            }
            "draw:frame" => self.frame(n, ctx, out)?,
            "text:page-number" | "text:page-count" | "text:date" => self.field(n, ctx, out),
            "text:text-input" => self.input(n, ctx, out),
            "text:note" => self.note(n, ctx, out)?,
            "text:note-ref" => self.note_ref(n, ctx, out),
            "office:annotation" => self.annotation(n, ctx),
            "office:annotation-end" => Self::annotation_end(n, ctx),
            "text:change-start" => self.change_start(n, ctx),
            "text:change-end" => Self::change_end(ctx),
            "text:change" => self.change(n, ctx, out)?,
            "text:bookmark" | "text:bookmark-start" | "text:bookmark-end" => {
                self.log.lose("a bookmark (ignored)");
            }
            "text:soft-page-break" => self.log.lose("a soft page break (ignored)"),
            other => {
                self.log.lose(&format!("{other} read as its text"));
                self.inlines(n, ctx, out)?;
            }
        }
        Ok(())
    }

    fn field(&mut self, n: &Node, ctx: &mut Ctx<'_>, out: &mut Vec<Inline>) {
        let field = match n.name.as_str() {
            "text:page-number" => FieldKind::PageNumber,
            "text:page-count" => FieldKind::PageCount,
            _ => {
                let style = n
                    .get("style:data-style-name")
                    .and_then(|s| self.index.date(self.part, s));
                let format = if let Some(s) = style {
                    crate::datestyle::read_date_style(s, &mut self.log)
                } else {
                    self.log
                        .lose("a date field without a data style read as yyyy-MM-dd");
                    "yyyy-MM-dd".to_owned()
                };
                FieldKind::Date { format }
            }
        };
        ctx.push(
            InlineContent::Field(Field {
                field,
                cached: raw_text(n),
            }),
            out,
        );
        ctx.space.visible();
    }

    fn input(&mut self, n: &Node, ctx: &mut Ctx<'_>, out: &mut Vec<Inline>) {
        match n.get("text:description").and_then(Tagged::decode) {
            Some(Tagged::Form(f)) => ctx.push(InlineContent::FormField(f), out),
            Some(Tagged::Signature(f)) => ctx.push(InlineContent::SignatureField(f), out),
            None => {
                self.log.lose("an input field read as its text");
                ctx.text(raw_text(n), out);
            }
        }
        ctx.space.visible();
    }
}

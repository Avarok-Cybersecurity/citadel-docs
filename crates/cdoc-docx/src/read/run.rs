//! Runs: text, tabs, breaks, footnote marks, drawings and the pieces of complex fields.

use super::field::FieldState;
use super::paragraph::{Context, ParaState};
use super::Reader;
use crate::error::DocxError;
use crate::props::read_rpr;
use crate::xml::El;
use cdoc_model::{BreakKind, FootnoteId, Inline, InlineContent, RunProperties, TextRun};

impl Reader<'_> {
    pub(super) fn run(
        &mut self,
        run: El<'_>,
        ctx: &Context,
        st: &mut ParaState,
    ) -> Result<(), DocxError> {
        let rpr = run
            .find("w:rPr")
            .map_or_else(RunProperties::none, |n| read_rpr(n, &mut self.log));
        let mut text = String::new();
        let mut non_text = false;
        for child in run.elems() {
            let name = child.name();
            if st.field.is_some() || name == "w:fldChar" {
                self.field_piece(child, ctx, st, &rpr);
                continue;
            }
            match name {
                "w:rPr"
                | "w:footnoteRef"
                | "w:commentReference"
                | "w:annotationRef"
                | "w:separator"
                | "w:continuationSeparator"
                | "w:lastRenderedPageBreak" => {}
                "w:t" | "w:delText" => text.push_str(&child.content()),
                "w:tab" => {
                    Self::flush(&mut text, &rpr, ctx, st);
                    st.inlines.push(Inline {
                        content: InlineContent::Tab,
                        marks: st.marks(ctx),
                    });
                    non_text = true;
                }
                "w:br" | "w:cr" => {
                    Self::flush(&mut text, &rpr, ctx, st);
                    let kind = match child.get("w:type") {
                        Some("page") => BreakKind::Page,
                        Some("column") => BreakKind::Column,
                        _ => BreakKind::Line,
                    };
                    st.inlines.push(Inline {
                        content: InlineContent::Break { kind },
                        marks: st.marks(ctx),
                    });
                    non_text = true;
                }
                "w:footnoteReference" => {
                    Self::flush(&mut text, &rpr, ctx, st);
                    match child.get("w:id").and_then(|n| n.parse().ok()) {
                        Some(id) => st.inlines.push(Inline {
                            content: InlineContent::FootnoteRef { id: FootnoteId(id) },
                            marks: st.marks(ctx),
                        }),
                        None => self.log.lose("a footnote reference without a usable id"),
                    }
                }
                "w:drawing" => {
                    Self::flush(&mut text, &rpr, ctx, st);
                    if let Some(image) = self.drawing(child)? {
                        st.inlines.push(Inline {
                            content: InlineContent::Image(image),
                            marks: st.marks(ctx),
                        });
                    }
                    non_text = true;
                }
                other => self.log.unknown(&format!("w:r/{other}")),
            }
        }
        Self::flush(&mut text, &rpr, ctx, st);
        let rpr_matters = rpr.style.is_some() || rpr != RunProperties::none();
        if non_text && rpr_matters {
            self.log.lose("run formatting on a non-text inline");
        }
        Ok(())
    }

    /// Emit pending text as one inline.
    pub(super) fn flush(text: &mut String, rpr: &RunProperties, ctx: &Context, st: &mut ParaState) {
        if text.is_empty() {
            return;
        }
        st.inlines.push(Inline {
            content: InlineContent::Text(TextRun {
                text: std::mem::take(text),
                properties: rpr.clone(),
            }),
            marks: st.marks(ctx),
        });
    }

    fn field_piece(
        &mut self,
        child: El<'_>,
        ctx: &Context,
        st: &mut ParaState,
        rpr: &RunProperties,
    ) {
        match (child.name(), st.field.as_mut()) {
            ("w:fldChar", None) => {
                if child.get("w:fldCharType") == Some("begin") {
                    st.field = Some(FieldState::begin(st.marks(ctx), rpr.clone()));
                }
            }
            ("w:fldChar", Some(field)) => match child.get("w:fldCharType") {
                Some("begin") => field.depth += 1,
                Some("separate") if field.depth == 1 => field.in_result = true,
                Some("end") => {
                    field.depth -= 1;
                    if field.depth == 0 {
                        let done = st.field.take();
                        if let Some(done) = done {
                            self.finish_field(done, st);
                        }
                    }
                }
                _ => {}
            },
            ("w:instrText" | "w:delInstrText", Some(field)) if !field.in_result => {
                field.instr.push_str(&child.content());
            }
            ("w:t" | "w:delText", Some(field)) if field.in_result && field.depth == 1 => {
                field.result.push_str(&child.content());
            }
            ("w:rPr", _) => {}
            (other, _) => self.log.unknown(&format!("field content w:r/{other}")),
        }
    }
}

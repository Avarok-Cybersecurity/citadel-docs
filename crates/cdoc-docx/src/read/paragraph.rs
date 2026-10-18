//! Paragraphs: properties, then content flattened from wrappers into marked inlines.

use super::field::FieldState;
use super::Reader;
use crate::error::DocxError;
use crate::props::read_ppr;
use crate::xml::El;
use cdoc_model::{
    CommentId, Inline, Link, Marks, Paragraph, ParagraphProperties, Revision, RevisionId,
    RevisionKind,
};

/// The wrappers an inline sits inside.
#[derive(Clone)]
pub(super) struct Context {
    pub link: Option<Link>,
    pub revision: Option<Revision>,
}

/// A paragraph being built.
pub(super) struct ParaState {
    pub inlines: Vec<Inline>,
    pub active: Vec<CommentId>,
    pub field: Option<FieldState>,
}

impl ParaState {
    pub(super) fn marks(&self, ctx: &Context) -> Marks {
        Marks {
            link: ctx.link.clone(),
            comments: self.active.clone(),
            revision: ctx.revision.clone(),
        }
    }
}

impl Reader<'_> {
    /// A paragraph, and the section properties riding in it, if any.
    pub(super) fn paragraph<'n>(
        &mut self,
        p: El<'n>,
    ) -> Result<(Paragraph, Option<El<'n>>), DocxError> {
        let ppr = p.find("w:pPr");
        let properties = ppr.map_or_else(ParagraphProperties::none, |n| read_ppr(n, &mut self.log));
        let sect = ppr.and_then(|n| n.find("w:sectPr"));
        let mut st = ParaState {
            inlines: Vec::new(),
            active: Vec::new(),
            field: None,
        };
        let ctx = Context {
            link: None,
            revision: None,
        };
        self.walk(p, &ctx, &mut st)?;
        if st.field.is_some() {
            self.log.lose("a field that never ends");
        }
        Ok((
            Paragraph {
                properties,
                inlines: st.inlines,
            },
            sect,
        ))
    }

    pub(super) fn walk(
        &mut self,
        parent: El<'_>,
        ctx: &Context,
        st: &mut ParaState,
    ) -> Result<(), DocxError> {
        for child in parent.elems() {
            match child.name() {
                "w:pPr" | "w:proofErr" | "w:lastRenderedPageBreak" | "w:sdtPr" | "w:sdtEndPr" => {}
                "w:r" => self.run(child, ctx, st)?,
                "w:hyperlink" => {
                    let link = self.link(child);
                    let inner = Context {
                        link: link.or_else(|| ctx.link.clone()),
                        revision: ctx.revision.clone(),
                    };
                    self.walk(child, &inner, st)?;
                }
                "w:ins" | "w:del" | "w:moveTo" | "w:moveFrom" => {
                    let inner = Context {
                        link: ctx.link.clone(),
                        revision: Some(self.revision(child, ctx)),
                    };
                    self.walk(child, &inner, st)?;
                }
                "w:commentRangeStart" => {
                    if let Some(id) = self.comment_id(child) {
                        st.active.push(id);
                    }
                }
                "w:commentRangeEnd" => {
                    if let Some(id) = self.comment_id(child) {
                        st.active.retain(|a| a != &id);
                    }
                }
                "w:sdt" => self.sdt_inline(child, ctx, st)?,
                "w:sdtContent" => self.walk(child, ctx, st)?,
                "w:fldSimple" => self.simple_field(child, ctx, st),
                "w:bookmarkStart" | "w:bookmarkEnd" => self.log.lose(child.name()),
                other => self.log.unknown(&format!("{}/{other}", parent.name())),
            }
        }
        Ok(())
    }

    fn comment_id(&self, node: El<'_>) -> Option<CommentId> {
        let n: usize = node.get("w:id")?.parse().ok()?;
        Some(CommentId::new(self.ids.comments.name(n)))
    }

    fn revision(&mut self, node: El<'_>, outer: &Context) -> Revision {
        if outer.revision.is_some() {
            self.log
                .lose("nested tracked changes; the innermost is kept");
        }
        let kind = match node.name() {
            "w:ins" | "w:moveTo" => RevisionKind::Insertion,
            _ => RevisionKind::Deletion,
        };
        if node.name().starts_with("w:move") {
            self.log
                .lose("tracked moves read as deletion and insertion");
        }
        let n: usize = node.get("w:id").and_then(|n| n.parse().ok()).unwrap_or(0);
        Revision {
            id: RevisionId::new(self.ids.revisions.name(n)),
            kind,
            author: node.get("w:author").unwrap_or_default().to_owned(),
            date: node.get("w:date").unwrap_or_default().to_owned(),
        }
    }

    fn link(&mut self, node: El<'_>) -> Option<Link> {
        let title = node.get("w:tooltip").map(str::to_owned);
        if let Some(anchor) = node.get("w:anchor") {
            return Some(Link {
                href: format!("#{anchor}"),
                title,
            });
        }
        let rel = node.get("r:id").and_then(|id| self.rels.by_id(id));
        match rel {
            Some(rel) if !rel.target.is_empty() => Some(Link {
                href: rel.target.clone(),
                title,
            }),
            _ => {
                self.log.lose("a hyperlink with no resolvable target");
                None
            }
        }
    }
}

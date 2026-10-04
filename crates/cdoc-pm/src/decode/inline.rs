//! Inline nodes and their marks → inlines. Character-formatting marks are only legal on text.

use crate::json::{attr, attr_opt, attrs_into, PmError, PmMark, PmNode};
use crate::names::{attr as attr_name, mark, node};
use cdoc_model::{
    BreakKind, Inline, InlineContent, Link, Marks, Revision, RevisionKind, RunProperties, TextRun,
};

/// One inline from a node.
pub(crate) fn decode_inline(node: &PmNode, path: &str) -> Result<Inline, PmError> {
    let (mut props, marks) = split_marks(&node.marks, path)?;
    let content = match node.kind.as_str() {
        node::TEXT => {
            let text = node
                .text
                .clone()
                .ok_or_else(|| PmError::new(path, "text node without text"))?;
            InlineContent::Text(TextRun {
                text,
                properties: props.take().unwrap_or_else(RunProperties::none),
            })
        }
        node::IMAGE => InlineContent::Image(attrs_into(&node.attrs, path)?),
        node::HARD_BREAK => InlineContent::Break {
            kind: BreakKind::Line,
        },
        node::PAGE_BREAK => InlineContent::Break {
            kind: BreakKind::Page,
        },
        node::COLUMN_BREAK => InlineContent::Break {
            kind: BreakKind::Column,
        },
        node::TAB => InlineContent::Tab,
        node::FOOTNOTE_REF => InlineContent::FootnoteRef {
            id: attr(&node.attrs, attr_name::ID, path)?,
        },
        node::FIELD => InlineContent::Field(attrs_into(&node.attrs, path)?),
        node::FORM_FIELD => InlineContent::FormField(attrs_into(&node.attrs, path)?),
        node::SIGNATURE_FIELD => InlineContent::SignatureField(attrs_into(&node.attrs, path)?),
        other => {
            return Err(PmError::new(path, format!("{other:?} is not an inline")));
        }
    };
    if props.is_some() {
        return Err(PmError::new(
            path,
            format!("character formatting marks on a {:?} node", node.kind),
        ));
    }
    Ok(Inline { content, marks })
}

/// Character-formatting marks (if any) and range marks, read in any order. Every mark but
/// `comment` may appear at most once.
fn split_marks(marks: &[PmMark], path: &str) -> Result<(Option<RunProperties>, Marks), PmError> {
    let mut run = RunProperties::none();
    let mut range = Marks::none();
    let mut seen: Vec<&str> = Vec::new();
    for m in marks {
        if m.kind != mark::COMMENT {
            if seen.contains(&m.kind.as_str()) {
                return Err(PmError::new(path, format!("duplicate mark {:?}", m.kind)));
            }
            seen.push(&m.kind);
        }
        let a = &m.attrs;
        match m.kind.as_str() {
            mark::CHARACTER_STYLE => run.style = Some(attr(a, attr_name::STYLE_ID, path)?),
            mark::TEXT_STYLE => {
                run.font = attr_opt(a, attr_name::FONT_FAMILY, path)?;
                run.size = attr_opt(a, attr_name::FONT_SIZE, path)?;
                run.color = attr_opt(a, attr_name::COLOR, path)?;
            }
            mark::BOLD => run.bold = Some(attr(a, attr_name::ON, path)?),
            mark::ITALIC => run.italic = Some(attr(a, attr_name::ON, path)?),
            mark::UNDERLINE => run.underline = Some(attrs_into(a, path)?),
            mark::STRIKE => run.strike = Some(attr(a, attr_name::ON, path)?),
            mark::VERTICAL_ALIGN => {
                run.vertical_align = Some(attr(a, attr_name::VALUE, path)?);
            }
            mark::HIGHLIGHT => run.highlight = Some(attr(a, attr_name::COLOR, path)?),
            mark::SMALL_CAPS => run.small_caps = Some(attr(a, attr_name::ON, path)?),
            mark::ALL_CAPS => run.all_caps = Some(attr(a, attr_name::ON, path)?),
            mark::LINK => range.link = Some(attrs_into::<Link>(a, path)?),
            mark::COMMENT => range.comments.push(attr(a, attr_name::ID, path)?),
            mark::INSERTION | mark::DELETION => {
                if range.revision.is_some() {
                    return Err(PmError::new(path, "both insertion and deletion marks"));
                }
                let kind = if m.kind == mark::INSERTION {
                    RevisionKind::Insertion
                } else {
                    RevisionKind::Deletion
                };
                range.revision = Some(Revision {
                    id: attr(a, attr_name::ID, path)?,
                    kind,
                    author: attr(a, "author", path)?,
                    date: attr(a, "date", path)?,
                });
            }
            other => return Err(PmError::new(path, format!("unknown mark {other:?}"))),
        }
    }
    Ok(((!run.is_none()).then_some(run), range))
}

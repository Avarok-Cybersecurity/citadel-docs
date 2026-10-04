//! Paragraph content as pieces: text or raw Markdown, each with the formatting Markdown has
//! (bold, italic, strikethrough, code, link). Everything else is reported.

use super::escape::{destination, escape};
use super::formatting::note_formatting;
use super::Ctx;
use cdoc_model::{BreakKind, FormFieldKind, Inline, InlineContent, Link, RevisionKind};

/// Monospace families that read back as code.
pub const MONOSPACE: [&str; 5] = ["Courier New", "Consolas", "Courier", "Menlo", "Monaco"];

/// The family a code span reads back as.
pub const CODE_FONT: &str = "Courier New";

/// The emphasis markers a run carries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Emphasis {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) strike: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Fmt {
    pub(super) emphasis: Emphasis,
    pub(super) code: bool,
    pub(super) link: Option<Link>,
}

impl Fmt {
    const fn plain() -> Self {
        Self {
            emphasis: Emphasis {
                bold: false,
                italic: false,
                strike: false,
            },
            code: false,
            link: None,
        }
    }

    pub(super) fn emphasis_free(&self) -> Self {
        Self {
            emphasis: Emphasis {
                bold: false,
                italic: false,
                strike: false,
            },
            ..self.clone()
        }
    }

    pub(super) fn markers(&self) -> Vec<Marker> {
        let mut out = Vec::new();
        if let Some(link) = &self.link {
            out.push(Marker::Link(link.clone()));
        }
        for (on, marker) in [
            (self.emphasis.strike, Marker::Strike),
            (self.emphasis.bold, Marker::Bold),
            (self.emphasis.italic, Marker::Italic),
        ] {
            if on {
                out.push(marker);
            }
        }
        out
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Marker {
    Link(Link),
    Bold,
    Strike,
    Italic,
}

pub(super) enum Content {
    Text(String),
    Raw(String),
    Break,
}

pub(super) struct Piece {
    pub(super) content: Content,
    pub(super) fmt: Fmt,
}

pub(super) fn pieces(inlines: &[Inline], ctx: &mut Ctx<'_>) -> Vec<Piece> {
    let mut out = Vec::new();
    for inline in inlines {
        if !inline.marks.comments.is_empty() {
            ctx.lose("comment anchor");
        }
        if let Some(rev) = &inline.marks.revision {
            match rev.kind {
                RevisionKind::Deletion => {
                    ctx.lose("tracked deletion (text removed)");
                    continue;
                }
                RevisionKind::Insertion => ctx.lose("tracked insertion (accepted)"),
            }
        }
        let mut fmt = Fmt {
            link: inline.marks.link.clone(),
            ..Fmt::plain()
        };
        let content = match &inline.content {
            InlineContent::Text(run) => {
                note_formatting(&run.properties, ctx);
                fmt.emphasis = Emphasis {
                    bold: run.properties.bold == Some(true),
                    italic: run.properties.italic == Some(true),
                    strike: run.properties.strike == Some(true),
                };
                fmt.code = run
                    .properties
                    .font
                    .as_ref()
                    .is_some_and(|f| MONOSPACE.contains(&f.as_str()));
                Content::Text(run.text.clone())
            }
            InlineContent::Image(img) => {
                let alt = escape(img.alt.as_deref().unwrap_or_default(), false);
                let dest = destination(&format!("cid:{}", img.content_id.as_str()));
                Content::Raw(format!("![{alt}]({dest})"))
            }
            InlineContent::Break {
                kind: BreakKind::Line,
            } => Content::Break,
            InlineContent::Break { .. } => {
                ctx.lose("page or column break");
                continue;
            }
            InlineContent::Tab => Content::Text("\t".to_owned()),
            InlineContent::FootnoteRef { id } => Content::Raw(format!("[^{}]", id.0)),
            InlineContent::Field(f) => {
                ctx.lose("field (its cached text is written)");
                Content::Text(f.cached.clone())
            }
            InlineContent::FormField(f) => {
                ctx.lose("form field (written as its value)");
                Content::Raw(match &f.field {
                    FormFieldKind::Checkbox { checked } => {
                        if *checked { "\\[x\\]" } else { "\\[ \\]" }.to_owned()
                    }
                    FormFieldKind::Text { value, .. } => escape(value, false),
                    FormFieldKind::Dropdown { options, selected } => escape(
                        selected
                            .and_then(|s| options.get(usize::try_from(s).ok()?))
                            .map_or("", String::as_str),
                        false,
                    ),
                    FormFieldKind::Date { value } => {
                        escape(value.as_deref().unwrap_or_default(), false)
                    }
                })
            }
            InlineContent::SignatureField(_) => {
                ctx.lose("signature field (dropped)");
                continue;
            }
        };
        out.push(Piece { content, fmt });
    }
    out
}

/// Move whitespace and punctuation at the edges of an emphasised run outside its markers:
/// CommonMark only opens emphasis before a word character and closes it after one.
pub(super) fn split_edges(piece: Piece, ctx: &mut Ctx<'_>) -> Vec<Piece> {
    let Content::Text(text) = &piece.content else {
        return vec![piece];
    };
    if piece.fmt.code
        || piece
            .fmt
            .markers()
            .iter()
            .all(|m| matches!(m, Marker::Link(_)))
    {
        return vec![piece];
    }
    let edge = |c: char| !c.is_alphanumeric();
    let lead_len = text.len() - text.trim_start_matches(edge).len();
    let rest = &text[lead_len..];
    let core_len = rest.trim_end_matches(edge).len();
    let (lead, core, trail) = (&text[..lead_len], &rest[..core_len], &rest[core_len..]);
    if lead
        .chars()
        .chain(trail.chars())
        .any(|c| !c.is_whitespace())
    {
        ctx.lose("emphasis on edge punctuation (moved outside the markers)");
    }
    let plain = piece.fmt.emphasis_free();
    let mut out = Vec::new();
    if !lead.is_empty() {
        out.push(Piece {
            content: Content::Text(lead.to_owned()),
            fmt: plain.clone(),
        });
    }
    if !core.is_empty() {
        out.push(Piece {
            content: Content::Text(core.to_owned()),
            fmt: piece.fmt.clone(),
        });
    }
    if !trail.is_empty() {
        out.push(Piece {
            content: Content::Text(trail.to_owned()),
            fmt: plain,
        });
    }
    out
}

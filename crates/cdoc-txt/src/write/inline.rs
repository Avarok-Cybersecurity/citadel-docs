//! Paragraph content as text.

use super::Ctx;
use cdoc_model::{BreakKind, FormFieldKind, Inline, InlineContent, RevisionKind, RunProperties};
use std::fmt::Write;

fn note_formatting(props: &RunProperties, ctx: &mut Ctx<'_>) {
    let mut p = props.clone();
    p.style = None;
    if !p.is_none() {
        ctx.lose("character formatting");
    }
}

/// The text of a paragraph's inlines. Hard line breaks become `\n`, page breaks form feeds.
pub(super) fn text(inlines: &[Inline], ctx: &mut Ctx<'_>) -> String {
    let mut out = String::new();
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
        let start = out.len();
        match &inline.content {
            InlineContent::Text(run) => {
                note_formatting(&run.properties, ctx);
                out.push_str(&run.text);
            }
            InlineContent::Tab => out.push('\t'),
            InlineContent::Break { kind } => out.push(match kind {
                BreakKind::Page => '\u{c}',
                BreakKind::Line | BreakKind::Column => '\n',
            }),
            InlineContent::Image(img) => {
                ctx.lose("image (written as a placeholder)");
                match &img.alt {
                    Some(alt) if !alt.is_empty() => {
                        let _ = write!(out, "[image: {alt}]");
                    }
                    _ => out.push_str("[image]"),
                }
            }
            InlineContent::FootnoteRef { id } => {
                let _ = write!(out, "[{}]", id.0);
            }
            InlineContent::Field(f) => {
                ctx.lose("field (its cached text is written)");
                out.push_str(&f.cached);
            }
            InlineContent::FormField(f) => {
                ctx.lose("form field (written as its value)");
                match &f.field {
                    FormFieldKind::Checkbox { checked } => {
                        out.push_str(if *checked { "[x]" } else { "[ ]" });
                    }
                    FormFieldKind::Text { value, .. } => out.push_str(value),
                    FormFieldKind::Dropdown { options, selected } => {
                        let chosen = selected.and_then(|s| options.get(usize::try_from(s).ok()?));
                        out.push_str(chosen.map_or("", String::as_str));
                    }
                    FormFieldKind::Date { value } => {
                        out.push_str(value.as_deref().unwrap_or_default());
                    }
                }
            }
            InlineContent::SignatureField(f) => {
                ctx.lose("signature field (written as a placeholder)");
                match &f.label {
                    Some(label) => {
                        let _ = write!(out, "[signature: {label}]");
                    }
                    None => {
                        let _ = write!(out, "[signature: {}]", f.signer_role);
                    }
                }
            }
        }
        if let Some(link) = &inline.marks.link {
            let shown = &out[start..];
            if shown != link.href && !link.href.starts_with('#') {
                let _ = write!(out, " ({})", link.href);
            }
        }
    }
    out
}

//! Blocks: headings and paragraphs, quotes, list items, rules, footnotes.

use super::inline::text;
use super::table::table;
use super::Ctx;
use cdoc_layout::labels::Counters;
use cdoc_model::{Block, Document, NumberFormat, Paragraph};

enum Chunk {
    Block(String),
    Item(String),
}

pub(super) fn paragraph_text(p: &Paragraph, ctx: &mut Ctx<'_>) -> String {
    text(&p.inlines, ctx)
}

fn is_rule(p: &Paragraph) -> bool {
    p.inlines.is_empty()
        && p.properties.numbering.is_none()
        && p.properties
            .borders
            .as_ref()
            .is_some_and(|b| b.bottom.is_some())
}

fn indent_lines(text: &str, pad: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                l.to_owned()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn paragraph(p: &Paragraph, ctx: &mut Ctx<'_>) -> Option<Chunk> {
    let pr = &p.properties;
    if pr.alignment.is_some() || pr.spacing.is_some() || pr.indent.is_some() || pr.shading.is_some()
    {
        ctx.lose("paragraph formatting (alignment, spacing, indent, shading)");
    }
    let reference = ctx
        .doc
        .styles
        .resolve_paragraph(pr)
        .ok()
        .and_then(|resolved| resolved.numbering);
    if let Some(reference) = reference {
        if let Some(label) = ctx.counters.next(reference) {
            let indent = "  ".repeat(usize::from(reference.level));
            let marker = match label.level.format {
                NumberFormat::Bullet if label.text.is_ascii() => label.text,
                NumberFormat::Bullet => "-".to_owned(),
                NumberFormat::None => String::new(),
                _ => label.text,
            };
            let body = paragraph_text(p, ctx);
            let line = if marker.is_empty() {
                format!("{indent}{body}")
            } else {
                let pad = " ".repeat(indent.len() + marker.chars().count() + 1);
                format!("{indent}{marker} {}", indent_lines(&body, &pad))
            };
            return Some(Chunk::Item(line));
        }
        ctx.lose("list reference that does not resolve (written as a paragraph)");
    }
    if is_rule(p) {
        return Some(Chunk::Block("---".to_owned()));
    }
    if p.inlines.is_empty() {
        ctx.lose("empty paragraph");
        return None;
    }
    let body = paragraph_text(p, ctx);
    if pr.style.as_ref().is_some_and(|s| s.as_str() == "Quote") {
        let quoted: Vec<String> = body.split('\n').map(|l| format!("> {l}")).collect();
        return Some(Chunk::Block(quoted.join("\n")));
    }
    Some(Chunk::Block(body))
}

fn blocks(list: &[Block], ctx: &mut Ctx<'_>) -> Vec<Chunk> {
    let mut out = Vec::new();
    for block in list {
        match block {
            Block::Paragraph(p) => out.extend(paragraph(p, ctx)),
            Block::Table(t) => out.push(Chunk::Block(table(t, ctx))),
        }
    }
    out
}

fn join(chunks: Vec<Chunk>) -> String {
    let mut out = String::new();
    let mut previous_item = false;
    for (i, chunk) in chunks.into_iter().enumerate() {
        let (body, item) = match chunk {
            Chunk::Block(t) => (t, false),
            Chunk::Item(t) => (t, true),
        };
        if i > 0 {
            out.push_str(if previous_item && item { "\n" } else { "\n\n" });
        }
        out.push_str(&body);
        previous_item = item;
    }
    out
}

pub(super) fn document(ctx: &mut Ctx<'_>) -> String {
    let doc: &Document = ctx.doc;
    let mut chunks = Vec::new();
    for section in &doc.sections {
        for _ in section.properties.headers.present() {
            ctx.lose("header");
        }
        for _ in section.properties.footers.present() {
            ctx.lose("footer");
        }
        chunks.extend(blocks(&section.blocks, ctx));
    }
    for note in &doc.footnotes {
        ctx.counters = Counters::new(&doc.numbering);
        let body: Vec<String> = blocks(&note.blocks, ctx)
            .into_iter()
            .map(|c| match c {
                Chunk::Block(t) | Chunk::Item(t) => t,
            })
            .collect();
        let label = format!("[{}] ", note.id.0);
        let pad = " ".repeat(label.chars().count());
        chunks.push(Chunk::Block(format!(
            "{label}{}",
            indent_lines(&body.join("\n\n"), &pad)
        )));
    }
    for _ in &doc.comments {
        ctx.lose("comment");
    }
    let mut out = join(chunks);
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

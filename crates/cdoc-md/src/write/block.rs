//! Blocks: headings, quotes, list items, rules, paragraphs, footnote definitions.

use super::inline::render;
use super::table::table;
use super::Ctx;
use cdoc_layout::labels::Counters;
use cdoc_model::{Block, Document, NumberFormat, NumberingRef, Paragraph};

/// A unit of output and how it joins its neighbours.
pub(super) enum Chunk {
    Block(String),
    Item(String),
}

fn style_of(p: &Paragraph) -> Option<&str> {
    p.properties.style.as_ref().map(cdoc_model::StyleId::as_str)
}

fn heading_level(style: &str) -> Option<usize> {
    if style == "Title" {
        return Some(1);
    }
    let n: usize = style.strip_prefix("Heading")?.parse().ok()?;
    (1..=6).contains(&n).then_some(n)
}

const KNOWN_STYLES: [&str; 8] = [
    "Normal",
    "Quote",
    "ListParagraph",
    "Header",
    "Footer",
    "FootnoteText",
    "Hyperlink",
    "BodyText",
];

fn note_paragraph_formatting(p: &Paragraph, ctx: &mut Ctx<'_>) {
    let props = &p.properties;
    if props.alignment.is_some()
        || props.spacing.is_some()
        || props.indent.is_some()
        || props.shading.is_some()
        || props.keep_with_next.is_some()
        || props.keep_lines.is_some()
        || props.page_break_before.is_some()
        || props.borders.is_some()
    {
        ctx.lose("paragraph formatting (alignment, spacing, indent, borders, shading, pagination)");
    }
    if let Some(style) = style_of(p) {
        if heading_level(style).is_none() && !KNOWN_STYLES.contains(&style) {
            ctx.lose("paragraph style other than headings, quote and list");
        }
        if style == "Title" {
            ctx.lose("Title style (written as a level 1 heading)");
        }
    }
}

fn is_rule(p: &Paragraph) -> bool {
    p.inlines.is_empty()
        && p.properties.numbering.is_none()
        && p.properties
            .borders
            .as_ref()
            .is_some_and(|b| b.bottom.is_some())
}

fn indent_continuations(text: &str, prefix: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.to_owned()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The list a paragraph is in, from its direct properties or its style.
fn list_reference(p: &Paragraph, doc: &Document) -> Option<NumberingRef> {
    doc.styles
        .resolve_paragraph(&p.properties)
        .ok()
        .and_then(|resolved| resolved.numbering)
}

pub(super) fn paragraph(p: &Paragraph, ctx: &mut Ctx<'_>) -> Option<Chunk> {
    note_paragraph_formatting(p, ctx);
    let reference = list_reference(p, ctx.doc);
    if reference.is_none() {
        ctx.list_level = None;
    }
    if let Some(reference) = reference {
        if let Some(label) = ctx.counters.next(reference) {
            // A nested item needs an item above it to nest in.
            let level = ctx
                .list_level
                .map_or(0, |above| usize::from(reference.level).min(above + 1));
            ctx.list_level = Some(level);
            let indent = "    ".repeat(level);
            let marker = match label.level.format {
                NumberFormat::Bullet => Some("-".to_owned()),
                NumberFormat::None => None,
                _ => Some(format!("{}.", label.number)),
            };
            let text = render(&p.inlines, true, true, ctx);
            return Some(match marker {
                Some(m) => {
                    let pad = " ".repeat(indent.len() + m.len() + 1);
                    Chunk::Item(format!("{indent}{m} {}", indent_continuations(&text, &pad)))
                }
                None => Chunk::Block(format!("{indent}{text}")),
            });
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
    let style = style_of(p);
    if let Some(level) = style.and_then(heading_level) {
        let text = render(&p.inlines, true, false, ctx);
        return Some(Chunk::Block(format!("{} {text}", "#".repeat(level))));
    }
    let text = render(&p.inlines, true, true, ctx);
    if style == Some("Quote") {
        let quoted: Vec<String> = text.split('\n').map(|l| format!("> {l}")).collect();
        return Some(Chunk::Block(quoted.join("\n")));
    }
    Some(Chunk::Block(text))
}

pub(super) fn blocks(blocks: &[Block], ctx: &mut Ctx<'_>) -> Vec<Chunk> {
    let mut out = Vec::new();
    for block in blocks {
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
        let (text, is_item) = match chunk {
            Chunk::Block(t) => (t, false),
            Chunk::Item(t) => (t, true),
        };
        if i > 0 {
            out.push_str(if previous_item && is_item {
                "\n"
            } else {
                "\n\n"
            });
        }
        out.push_str(&text);
        previous_item = is_item;
    }
    out
}

pub(super) fn document(ctx: &mut Ctx<'_>) -> String {
    let doc: &Document = ctx.doc;
    let mut chunks = Vec::new();
    if doc.meta.title.is_some() {
        ctx.lose("document title (metadata)");
    }
    for section in &doc.sections {
        let props = &section.properties;
        ctx.lose("section page setup");
        for (_, _) in props.headers.present() {
            ctx.lose("header");
        }
        for (_, _) in props.footers.present() {
            ctx.lose("footer");
        }
        chunks.extend(blocks(&section.blocks, ctx));
    }
    for note in &doc.footnotes {
        ctx.counters = Counters::new(&doc.numbering);
        let mut inner = blocks(&note.blocks, ctx).into_iter().map(|c| match c {
            Chunk::Block(t) | Chunk::Item(t) => t,
        });
        let first = inner.next().unwrap_or_default();
        let rest: Vec<String> = inner
            .map(|t| format!("    {}", indent_continuations(&t, "    ")))
            .collect();
        let mut text = format!("[^{}]: {}", note.id.0, indent_continuations(&first, "    "));
        for r in rest {
            text.push_str("\n\n");
            text.push_str(&r);
        }
        chunks.push(Chunk::Block(text));
    }
    for _ in &doc.comments {
        ctx.lose("comment");
    }
    let mut text = join(chunks);
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

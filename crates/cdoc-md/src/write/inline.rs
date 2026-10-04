//! Paragraph content as Markdown inline syntax.
//!
//! Emphasis is written with `*`, `**` and `~~`. Where two delimiter runs would touch (one
//! span ending as another begins, or `**` against `~~`), CommonMark's flanking rules make the
//! result ambiguous, so those spans are written as HTML tags (`<strong>`, `<em>`, `<del>`),
//! which the reader accepts and no flanking rule applies to.

use super::escape::{code_span, destination, escape, title};
use super::pieces::{pieces, split_edges, Content, Marker, Piece};
use super::Ctx;
use cdoc_model::Inline;

/// An open or close of a span, or a piece of content.
enum Op {
    Open(usize),
    Close(usize),
    Piece(usize),
}

struct Span {
    marker: Marker,
    html: bool,
}

impl Span {
    const fn emphasis(&self) -> bool {
        !matches!(self.marker, Marker::Link(_))
    }

    /// The delimiter character an emphasis span would use.
    const fn delimiter(&self) -> char {
        match self.marker {
            Marker::Strike => '~',
            _ => '*',
        }
    }
}

fn open(span: &Span) -> &'static str {
    match (&span.marker, span.html) {
        (Marker::Link(_), _) => "[",
        (Marker::Bold, false) => "**",
        (Marker::Bold, true) => "<strong>",
        (Marker::Italic, false) => "*",
        (Marker::Italic, true) => "<em>",
        (Marker::Strike, false) => "~~",
        (Marker::Strike, true) => "<del>",
    }
}

fn close(span: &Span) -> String {
    match (&span.marker, span.html) {
        (Marker::Link(l), _) => {
            let title = l
                .title
                .as_deref()
                .map(|t| format!(" {}", title(t)))
                .unwrap_or_default();
            format!("]({}{title})", destination(&l.href))
        }
        (Marker::Bold, true) => "</strong>".to_owned(),
        (Marker::Italic, true) => "</em>".to_owned(),
        (Marker::Strike, true) => "</del>".to_owned(),
        _ => open(span).to_owned(),
    }
}

/// The opens and closes between pieces, from a stack discipline: close what the next piece
/// does not want (and anything opened above it), then open what is missing.
fn schedule(all: &[Piece]) -> (Vec<Span>, Vec<Op>) {
    let mut spans: Vec<Span> = Vec::new();
    let mut ops = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for (i, piece) in all.iter().enumerate() {
        let wanted = if matches!(piece.content, Content::Break) {
            Vec::new()
        } else {
            piece.fmt.markers()
        };
        let keep = stack
            .iter()
            .zip(&wanted)
            .take_while(|(s, w)| spans[**s].marker == **w)
            .count();
        for s in stack.drain(keep..).rev() {
            ops.push(Op::Close(s));
        }
        for marker in &wanted[keep..] {
            spans.push(Span {
                marker: marker.clone(),
                html: false,
            });
            stack.push(spans.len() - 1);
            ops.push(Op::Open(spans.len() - 1));
        }
        ops.push(Op::Piece(i));
    }
    for s in stack.drain(..).rev() {
        ops.push(Op::Close(s));
    }
    (spans, ops)
}

/// Spans that open together must close together: `***a*b***` is parsed unreliably, so when
/// they do not, they are written as tags.
fn resolve_split_groups(spans: &mut [Span], ops: &[Op]) {
    let mut boundary = 0;
    let mut opened: Vec<(usize, usize)> = Vec::new();
    let mut closed: Vec<(usize, usize)> = Vec::new();
    for op in ops {
        match op {
            Op::Piece(_) => boundary += 1,
            Op::Open(s) if spans[*s].emphasis() => opened.push((*s, boundary)),
            Op::Close(s) if spans[*s].emphasis() => closed.push((*s, boundary)),
            _ => {}
        }
    }
    let closes_at = |s: usize| closed.iter().find(|(c, _)| *c == s).map(|(_, b)| *b);
    for (s, at) in &opened {
        let group: Vec<usize> = opened
            .iter()
            .filter(|(_, b)| b == at)
            .map(|(s, _)| *s)
            .collect();
        if group.len() > 1 && group.iter().any(|g| closes_at(*g) != closes_at(*s)) {
            for g in group {
                spans[g].html = true;
            }
        }
    }
}

/// Mark spans html where their delimiter runs would touch another's.
fn resolve_touching(spans: &mut [Span], ops: &[Op]) {
    let mut at = 0;
    while at < ops.len() {
        let end = ops[at..]
            .iter()
            .position(|o| matches!(o, Op::Piece(_)))
            .map_or(ops.len(), |p| at + p);
        let boundary = &ops[at..end];
        let emphasis = |close: bool| -> Vec<usize> {
            boundary
                .iter()
                .filter_map(|o| match (o, close) {
                    (Op::Close(s), true) | (Op::Open(s), false) if spans[*s].emphasis() => Some(*s),
                    _ => None,
                })
                .collect()
        };
        let (closing, opening) = (emphasis(true), emphasis(false));
        let mixed = |ids: &[usize]| {
            ids.iter()
                .any(|s| spans[*s].delimiter() != spans[ids[0]].delimiter())
        };
        if (!closing.is_empty() && !opening.is_empty()) || mixed(&closing) || mixed(&opening) {
            for s in closing.into_iter().chain(opening) {
                spans[s].html = true;
            }
        }
        at = end + 1;
    }
}

/// Render inlines. `line_start` says the first character begins a line of block content;
/// `keep_breaks` writes hard breaks, otherwise they become spaces (headings).
pub(super) fn render(
    inlines: &[Inline],
    line_start: bool,
    keep_breaks: bool,
    ctx: &mut Ctx<'_>,
) -> String {
    let mut all: Vec<Piece> = Vec::new();
    for piece in pieces(inlines, ctx)
        .into_iter()
        .flat_map(|p| split_edges(p, ctx))
    {
        // Adjacent text with equal formatting is one run; two code spans side by side would
        // otherwise read back as one span holding the backticks.
        if let (
            Some(Piece {
                content: Content::Text(prev),
                fmt,
            }),
            Content::Text(next),
        ) = (all.last_mut(), &piece.content)
        {
            if *fmt == piece.fmt {
                prev.push_str(next);
                continue;
            }
        }
        all.push(piece);
    }
    let (mut spans, ops) = schedule(&all);
    resolve_touching(&mut spans, &ops);
    resolve_split_groups(&mut spans, &ops);
    let mut out = String::new();
    let mut at_start = line_start;
    for op in &ops {
        match op {
            Op::Open(s) => out.push_str(open(&spans[*s])),
            Op::Close(s) => out.push_str(&close(&spans[*s])),
            Op::Piece(i) => {
                let piece = &all[*i];
                match &piece.content {
                    Content::Text(t) if piece.fmt.code => {
                        let span = code_span(t);
                        out.push_str(&if ctx.in_table {
                            span.replace('|', "\\|")
                        } else {
                            span
                        });
                    }
                    Content::Text(t) => out.push_str(&escape(&t.replace('\n', " "), at_start)),
                    Content::Raw(r) => out.push_str(r),
                    Content::Break if keep_breaks => out.push_str("\\\n"),
                    Content::Break => out.push(' '),
                }
                at_start = out.ends_with('\n');
            }
        }
    }
    // Trailing spaces at the end of a paragraph are stripped by every parser.
    if let Some(last) = out.pop() {
        match last {
            ' ' => out.push_str("&#32;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

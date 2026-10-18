//! Reading a list style: levels in order (at most nine), label templates from prefix, shown
//! levels and suffix, indents in the label-alignment mode.

use super::{unspell, FORMATS};
use crate::props::{read_run, TEXT_PROPERTIES};
use crate::units;
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, FirstLine, Indent, Length, Level, NumberFormat,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;
use std::collections::BTreeMap;

fn read_indent(props: &Node, log: &mut Log) -> Option<Indent> {
    let label = props.find("style:list-level-label-alignment");
    let Some(label) = label else {
        if props.get("text:space-before").is_some() || props.get("text:min-label-width").is_some() {
            log.lose("list indents in the old position mode");
        }
        return None;
    };
    let left = label
        .get("fo:margin-left")
        .and_then(units::length)
        .map(Length);
    let first_line = label
        .get("fo:text-indent")
        .and_then(units::signed_length)
        .map(|(v, neg)| {
            if neg {
                FirstLine::Hanging { by: Length(-v) }
            } else {
                FirstLine::Indent { by: Length(v) }
            }
        });
    (left.is_some() || first_line.is_some()).then_some(Indent {
        left,
        right: None,
        first_line,
    })
}

fn read_level(
    node: &Node,
    number: usize,
    faces: &BTreeMap<String, String>,
    log: &mut Log,
) -> Level {
    let props = node.find("style:list-level-properties");
    let alignment = match props.and_then(|p| p.get("fo:text-align")) {
        Some("center") => Alignment::Center,
        Some("end" | "right") => Alignment::Right,
        Some("justify") => Alignment::Justify,
        _ => Alignment::Left,
    };
    let indent = props.and_then(|p| read_indent(p, log));
    let run = node.find(TEXT_PROPERTIES).map(|t| read_run(t, faces, log));
    if node.get("text:style-name").is_some() {
        log.lose("a list label's character style");
    }
    let (format, text, start) = if node.name == "text:list-level-style-bullet" {
        let glyph = node
            .get("text:bullet-char")
            .unwrap_or("\u{2022}")
            .to_owned();
        (NumberFormat::Bullet, glyph, 1)
    } else if node.name == "text:list-level-style-number" {
        let raw = node.get("style:num-format").unwrap_or("");
        let format = FORMATS.iter().find(|(_, s)| *s == raw).map_or_else(
            || {
                log.lose(&format!("list number format {raw:?} read as decimal"));
                NumberFormat::Decimal
            },
            |(f, _)| *f,
        );
        let shown = node
            .get("text:display-levels")
            .and_then(|d| d.parse().ok())
            .unwrap_or(1);
        let prefix = node.get("style:num-prefix").unwrap_or("");
        let suffix = node.get("style:num-suffix").unwrap_or("");
        let start = node
            .get("text:start-value")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        (format, unspell(prefix, shown, suffix, number), start)
    } else {
        log.lose(&format!("{} read as a number-less level", node.name));
        (NumberFormat::None, String::new(), 1)
    };
    Level {
        start,
        format,
        text,
        alignment,
        indent,
        run,
    }
}

/// Read a list style as definition `id`; at most nine levels are kept.
pub fn read_list_style(
    node: &Node,
    id: AbstractNumId,
    faces: &BTreeMap<String, String>,
    log: &mut Log,
) -> AbstractNumbering {
    let mut levels: Vec<(usize, &Node)> = node
        .elems()
        .filter_map(|l| Some((l.get("text:level")?.parse::<usize>().ok()?, l)))
        .collect();
    levels.sort_by_key(|(n, _)| *n);
    let mut out = Vec::new();
    for (number, level) in levels {
        if number != out.len() + 1 || out.len() == 9 {
            log.lose("a list level beyond the ninth or out of sequence");
            continue;
        }
        out.push(read_level(level, number, faces, log));
    }
    if out.is_empty() {
        log.lose("a list style with no levels; one decimal level assumed");
        out.push(read_level(
            &Node::new("text:list-level-style-number").attr("style:num-format", "1"),
            1,
            faces,
            log,
        ));
    }
    AbstractNumbering { id, levels: out }
}

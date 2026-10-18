//! `style:paragraph-properties` and [`ParagraphProperties`]. The style, list membership and
//! outline level are carried by the document structure, not here.

use super::{border, color_attr, keep, read_keep, report_children, write_color};
use crate::units;
use cdoc_model::{
    Alignment, FirstLine, Indent, Length, LineSpacing, ParagraphProperties, Shading, Spacing,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

/// The element name.
pub const PARAGRAPH_PROPERTIES: &str = "style:paragraph-properties";

fn alignment_name(a: Alignment) -> &'static str {
    match a {
        Alignment::Left => "start",
        Alignment::Center => "center",
        Alignment::Right => "end",
        Alignment::Justify => "justify",
    }
}

fn length_attr(node: Node, key: &str, value: Option<Length>) -> Node {
    match value {
        Some(l) => node.attr(key, &units::write_length(l.0)),
        None => node,
    }
}

fn write_spacing(mut node: Node, s: &Spacing) -> Node {
    node = length_attr(node, "fo:margin-top", s.before);
    node = length_attr(node, "fo:margin-bottom", s.after);
    match s.line {
        Some(LineSpacing::Multiple { lines }) => {
            node.attr("fo:line-height", &units::write_line_multiple(lines))
        }
        Some(LineSpacing::Exact { height }) => {
            node.attr("fo:line-height", &units::write_length(height.0))
        }
        Some(LineSpacing::AtLeast { height }) => {
            node.attr("style:line-height-at-least", &units::write_length(height.0))
        }
        None => node,
    }
}

fn write_indent(mut node: Node, i: &Indent) -> Node {
    node = length_attr(node, "fo:margin-left", i.left);
    node = length_attr(node, "fo:margin-right", i.right);
    match i.first_line {
        Some(FirstLine::Indent { by }) => {
            node.attr("fo:text-indent", &units::write_signed_length(by.0, false))
        }
        Some(FirstLine::Hanging { by }) => {
            node.attr("fo:text-indent", &units::write_signed_length(by.0, true))
        }
        None => node,
    }
}

/// The element for paragraph formatting; `style`, `numbering` and `outline_level` are ignored.
#[must_use]
pub fn write_paragraph(p: &ParagraphProperties) -> Node {
    let mut node = Node::new(PARAGRAPH_PROPERTIES);
    if let Some(a) = p.alignment {
        node = node.attr("fo:text-align", alignment_name(a));
    }
    if let Some(s) = &p.spacing {
        node = write_spacing(node, s);
    }
    if let Some(i) = &p.indent {
        node = write_indent(node, i);
    }
    if let Some(b) = &p.borders {
        node = border::write_borders(node, b);
    }
    if let Some(s) = &p.shading {
        node = node.attr("fo:background-color", &write_color(s.fill));
    }
    for (key, value) in [
        ("fo:keep-with-next", keep(p.keep_with_next)),
        ("fo:keep-together", keep(p.keep_lines)),
        (
            "fo:break-before",
            p.page_break_before.map(|b| if b { "page" } else { "auto" }),
        ),
    ] {
        if let Some(value) = value {
            node = node.attr(key, value);
        }
    }
    node
}

fn read_alignment(value: &str) -> Option<Alignment> {
    Some(match value {
        "start" | "left" => Alignment::Left,
        "center" => Alignment::Center,
        "end" | "right" => Alignment::Right,
        "justify" => Alignment::Justify,
        _ => return None,
    })
}

/// The line rule, `Err` when a value is unreadable.
fn read_line(get: &dyn Fn(&str) -> Option<String>) -> Result<Option<LineSpacing>, ()> {
    if let Some(v) = get("fo:line-height") {
        if v == "normal" {
            return Ok(Some(LineSpacing::Multiple { lines: 240 }));
        }
        if v.trim_end().ends_with('%') {
            return units::line_multiple(&v)
                .map(|lines| Some(LineSpacing::Multiple { lines }))
                .ok_or(());
        }
        return units::length(&v)
            .map(|h| Some(LineSpacing::Exact { height: Length(h) }))
            .ok_or(());
    }
    if let Some(v) = get("style:line-height-at-least") {
        return units::length(&v)
            .map(|h| Some(LineSpacing::AtLeast { height: Length(h) }))
            .ok_or(());
    }
    Ok(None)
}

/// Read the element. `known` names attributes the caller maps itself (a master page's
/// page-number restart, the default tab stop) so they are not reported.
pub fn read_paragraph(node: &Node, known: &[&str], log: &mut Log) -> ParagraphProperties {
    let get = |key: &str| node.get(key).map(str::to_owned);
    let length = |key: &str, log: &mut Log| {
        let raw = node.get(key)?;
        let parsed = units::length(raw).map(Length);
        if parsed.is_none() {
            log.lose(&format!("unreadable value of {PARAGRAPH_PROPERTIES}@{key}"));
        }
        parsed
    };
    let mut p = ParagraphProperties::none();
    p.alignment = node.get("fo:text-align").and_then(|v| {
        let a = read_alignment(v);
        if a.is_none() {
            log.lose(&format!("text alignment {v}"));
        }
        a
    });
    let line = read_line(&get).unwrap_or_else(|()| {
        log.lose("unreadable line height");
        None
    });
    let (before, after) = (
        length("fo:margin-top", log),
        length("fo:margin-bottom", log),
    );
    if before.is_some() || after.is_some() || line.is_some() {
        p.spacing = Some(Spacing {
            before,
            after,
            line,
        });
    }
    let first_line = node.get("fo:text-indent").and_then(|raw| {
        let Some((v, negative)) = units::signed_length(raw) else {
            log.lose(&format!(
                "unreadable value of {PARAGRAPH_PROPERTIES}@fo:text-indent"
            ));
            return None;
        };
        Some(if negative {
            FirstLine::Hanging { by: Length(-v) }
        } else {
            FirstLine::Indent { by: Length(v) }
        })
    });
    let (left, right) = (
        length("fo:margin-left", log),
        length("fo:margin-right", log),
    );
    if left.is_some() || right.is_some() || first_line.is_some() {
        p.indent = Some(Indent {
            left,
            right,
            first_line,
        });
    }
    p.borders = border::read_borders(&|k| node.get(k), log);
    let mut fill = None;
    if let Some(v) = node.get("fo:background-color") {
        if v != "transparent" && !color_attr(v, &mut fill) {
            log.lose("unreadable paragraph background");
        }
    }
    p.shading = fill.map(|fill| Shading { fill });
    p.keep_with_next = node.get("fo:keep-with-next").and_then(read_keep);
    p.keep_lines = node.get("fo:keep-together").and_then(read_keep);
    p.page_break_before = match node.get("fo:break-before") {
        Some("page") => Some(true),
        Some("auto") => Some(false),
        Some(other) => {
            log.lose(&format!("a {other} break before a paragraph"));
            None
        }
        None => None,
    };
    for (key, _) in &node.attrs {
        if !MAPPED.contains(&key.as_str())
            && !border::KEYS.contains(&key.as_str())
            && !known.contains(&key.as_str())
        {
            log.lose(&format!("{PARAGRAPH_PROPERTIES}@{key}"));
        }
    }
    report_children(node, log);
    p
}

const MAPPED: [&str; 12] = [
    "fo:text-align",
    "fo:line-height",
    "style:line-height-at-least",
    "fo:margin-top",
    "fo:margin-bottom",
    "fo:text-indent",
    "fo:margin-left",
    "fo:margin-right",
    "fo:background-color",
    "fo:keep-with-next",
    "fo:keep-together",
    "fo:break-before",
];

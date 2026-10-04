//! Paragraph properties: `w:pPr`.

use super::{
    alignment_from, alignment_name, length, num, read_borders, read_shading, twips, write_borders,
    write_shading, BetweenEdge,
};
use crate::report::Log;
use crate::xml::{on_off, Node};
use cdoc_model::{
    FirstLine, Indent, Length, LineSpacing, NumId, NumberingRef, ParagraphProperties, Spacing,
    StyleId,
};

/// `<w:ind>` for an indent; also used inside numbering levels.
#[must_use]
pub fn write_indent(indent: &Indent, log: &mut Log) -> Node {
    let mut node = Node::new("w:ind");
    if let Some(left) = indent.left {
        node = node.num("w:left", twips(left, log));
    }
    if let Some(right) = indent.right {
        node = node.num("w:right", twips(right, log));
    }
    match indent.first_line {
        Some(FirstLine::Indent { by }) => node = node.num("w:firstLine", twips(by, log)),
        Some(FirstLine::Hanging { by }) => node = node.num("w:hanging", twips(by, log)),
        None => {}
    }
    node
}

/// Read `<w:ind>`.
pub fn read_indent(node: &Node, log: &mut Log) -> Indent {
    let first_line = if let Some(by) = length(node, "w:hanging", log) {
        Some(FirstLine::Hanging { by })
    } else {
        length(node, "w:firstLine", log).map(|by| FirstLine::Indent { by })
    };
    let left = length(node, "w:left", log).or_else(|| length(node, "w:start", log));
    let right = length(node, "w:right", log).or_else(|| length(node, "w:end", log));
    Indent {
        left,
        right,
        first_line,
    }
}

fn write_spacing(spacing: &Spacing, log: &mut Log) -> Node {
    let mut node = Node::new("w:spacing");
    if let Some(before) = spacing.before {
        node = node.num("w:before", twips(before, log));
    }
    if let Some(after) = spacing.after {
        node = node.num("w:after", twips(after, log));
    }
    match spacing.line {
        Some(LineSpacing::Multiple { lines }) => {
            node = node.num("w:line", lines).attr("w:lineRule", "auto");
        }
        Some(LineSpacing::Exact { height }) => {
            node = node
                .num("w:line", twips(height, log))
                .attr("w:lineRule", "exact");
        }
        Some(LineSpacing::AtLeast { height }) => {
            node = node
                .num("w:line", twips(height, log))
                .attr("w:lineRule", "atLeast");
        }
        None => {}
    }
    node
}

fn read_spacing(node: &Node, log: &mut Log) -> Spacing {
    let line = match (node.get("w:lineRule"), num::<i32>(node, "w:line", log)) {
        (Some("exact"), Some(h)) => Some(LineSpacing::Exact {
            height: Length::from_twips(h),
        }),
        (Some("atLeast"), Some(h)) => Some(LineSpacing::AtLeast {
            height: Length::from_twips(h),
        }),
        (_, Some(n)) => u32::try_from(n)
            .ok()
            .map(|lines| LineSpacing::Multiple { lines }),
        (_, None) => None,
    };
    Spacing {
        before: length(node, "w:before", log),
        after: length(node, "w:after", log),
        line,
    }
}

/// The `w:pPr` for these properties, children in schema order; always an element.
#[must_use]
pub fn write_ppr(props: &ParagraphProperties, log: &mut Log) -> Node {
    let mut node = Node::new("w:pPr");
    if let Some(style) = &props.style {
        node = node.child(Node::val("w:pStyle", style.as_str()));
    }
    let flags = [
        ("w:keepNext", props.keep_with_next),
        ("w:keepLines", props.keep_lines),
        ("w:pageBreakBefore", props.page_break_before),
    ];
    for (name, flag) in flags {
        if let Some(on) = flag {
            node = node.child(on_off(name, on));
        }
    }
    if let Some(r) = props.numbering {
        node = node.child(
            Node::new("w:numPr")
                .child(Node::val("w:ilvl", &r.level.to_string()))
                .child(Node::val("w:numId", &r.num_id.0.to_string())),
        );
    }
    if let Some(borders) = &props.borders {
        node = node.child(write_borders(
            "w:pBdr",
            borders,
            BetweenEdge::Paragraph,
            log,
        ));
    }
    if let Some(shading) = &props.shading {
        node = node.child(write_shading(shading));
    }
    if let Some(spacing) = &props.spacing {
        node = node.child(write_spacing(spacing, log));
    }
    if let Some(indent) = &props.indent {
        node = node.child(write_indent(indent, log));
    }
    if let Some(a) = props.alignment {
        node = node.child(Node::val("w:jc", alignment_name(a)));
    }
    if let Some(level) = props.outline_level {
        node = node.child(Node::val("w:outlineLvl", &level.to_string()));
    }
    node
}

/// Read a `w:pPr`. `w:sectPr` is the caller's business and is skipped here.
pub fn read_ppr(node: &Node, log: &mut Log) -> ParagraphProperties {
    let mut p = ParagraphProperties::none();
    for child in node.elems() {
        match child.name.as_str() {
            "w:pStyle" => p.style = child.value().map(StyleId::new),
            "w:keepNext" => p.keep_with_next = Some(child.is_on()),
            "w:keepLines" => p.keep_lines = Some(child.is_on()),
            "w:pageBreakBefore" => p.page_break_before = Some(child.is_on()),
            "w:numPr" => p.numbering = read_numbering_ref(child, log),
            "w:pBdr" => p.borders = Some(read_borders(child, BetweenEdge::Paragraph, log)),
            "w:shd" => p.shading = read_shading(child, log),
            "w:spacing" => p.spacing = Some(read_spacing(child, log)),
            "w:ind" => p.indent = Some(read_indent(child, log)),
            "w:jc" => p.alignment = child.value().and_then(alignment_from),
            "w:outlineLvl" => p.outline_level = num(child, "w:val", log).filter(|l| *l <= 8),
            "w:sectPr" => {}
            "w:rPr" => log.lose("w:pPr/w:rPr (paragraph mark formatting)"),
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    p
}

fn read_numbering_ref(node: &Node, log: &mut Log) -> Option<NumberingRef> {
    let num_id: u32 = num(node.find("w:numId")?, "w:val", log)?;
    if num_id == 0 {
        log.lose("w:numId 0 (list switched off) ignored");
        return None;
    }
    let level = node
        .find("w:ilvl")
        .and_then(|n| num(n, "w:val", log))
        .unwrap_or(0);
    Some(NumberingRef {
        num_id: NumId(num_id),
        level,
    })
}

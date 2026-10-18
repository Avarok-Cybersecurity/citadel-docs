//! Run properties: `w:rPr`.

use super::border::{hex6, parse_color};
use super::{
    num, read_shading, underline_from, underline_name, vertical_align_from, vertical_align_name,
    write_shading,
};
use crate::report::Log;
use crate::xml::{on_off, El, Node, WordEl, WordNode};
use cdoc_model::{Color, FontSize, RunProperties, Shading, StyleId, Underline};

/// Word's sixteen highlight colours. Other colours are written as run shading instead.
const HIGHLIGHTS: [(&str, [u8; 3]); 16] = [
    ("black", [0, 0, 0]),
    ("blue", [0, 0, 0xFF]),
    ("cyan", [0, 0xFF, 0xFF]),
    ("green", [0, 0xFF, 0]),
    ("magenta", [0xFF, 0, 0xFF]),
    ("red", [0xFF, 0, 0]),
    ("yellow", [0xFF, 0xFF, 0]),
    ("white", [0xFF, 0xFF, 0xFF]),
    ("darkBlue", [0, 0, 0x80]),
    ("darkCyan", [0, 0x80, 0x80]),
    ("darkGreen", [0, 0x80, 0]),
    ("darkMagenta", [0x80, 0, 0x80]),
    ("darkRed", [0x80, 0, 0]),
    ("darkYellow", [0x80, 0x80, 0]),
    ("darkGray", [0x80, 0x80, 0x80]),
    ("lightGray", [0xC0, 0xC0, 0xC0]),
];

fn highlight_name(color: Color) -> Option<&'static str> {
    HIGHLIGHTS
        .iter()
        .find(|(_, [r, g, b])| Color::rgb(*r, *g, *b) == color)
        .map(|(name, _)| *name)
}

fn highlight_color(name: &str) -> Option<Color> {
    HIGHLIGHTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, [r, g, b])| Color::rgb(*r, *g, *b))
}

/// The `w:rPr` for these properties, children in schema order. Always an element, possibly
/// empty: callers decide whether an empty one is worth writing.
#[must_use]
pub fn write_rpr(props: &RunProperties) -> Node {
    let mut node = Node::new("w:rPr");
    if let Some(style) = &props.style {
        node = node.child(Node::val("w:rStyle", style.as_str()));
    }
    if let Some(font) = &props.font {
        node = node.child(
            Node::new("w:rFonts")
                .attr("w:ascii", font)
                .attr("w:hAnsi", font)
                .attr("w:eastAsia", font)
                .attr("w:cs", font),
        );
    }
    let flags = [
        ("w:b", props.bold),
        ("w:i", props.italic),
        ("w:caps", props.all_caps),
        ("w:smallCaps", props.small_caps),
        ("w:strike", props.strike),
    ];
    for (name, flag) in flags {
        if let Some(on) = flag {
            node = node.child(on_off(name, on));
        }
    }
    if let Some(color) = props.color {
        node = node.child(Node::val("w:color", &hex6(color)));
    }
    if let Some(size) = props.size {
        node = node.child(Node::val("w:sz", &size.0.to_string()));
    }
    let named = props.highlight.and_then(highlight_name);
    if let Some(name) = named {
        node = node.child(Node::val("w:highlight", name));
    }
    if let Some(Underline { style, color }) = &props.underline {
        let u = Node::val("w:u", underline_name(*style));
        node = node.child(match color {
            Some(c) => u.attr("w:color", &hex6(*c)),
            None => u,
        });
    }
    if let (Some(fill), None) = (props.highlight, named) {
        node = node.child(write_shading(&Shading { fill }));
    }
    if let Some(v) = props.vertical_align {
        node = node.child(Node::val("w:vertAlign", vertical_align_name(v)));
    }
    node
}

/// Read a `w:rPr` (or the run half of a style).
pub fn read_rpr(node: El<'_>, log: &mut Log) -> RunProperties {
    read_rpr_skipping(node, None, log)
}

/// Read a `w:rPr` without its child element number `skip`, which the caller has taken (the
/// document defaults' `w:lang`, which the model keeps in the metadata).
pub fn read_rpr_skipping(node: El<'_>, skip: Option<usize>, log: &mut Log) -> RunProperties {
    let mut p = RunProperties::none();
    for (_, child) in node.elems().enumerate().filter(|(i, _)| Some(*i) != skip) {
        match child.name() {
            "w:rStyle" => p.style = child.value().map(StyleId::new),
            "w:rFonts" => {
                p.font = child
                    .get("w:ascii")
                    .or_else(|| child.get("w:hAnsi"))
                    .or_else(|| child.get("w:eastAsia"))
                    .map(str::to_owned);
                if p.font.is_none() {
                    log.lose("w:rFonts without a named font (theme fonts)");
                }
            }
            "w:b" => p.bold = Some(child.is_on()),
            "w:i" => p.italic = Some(child.is_on()),
            "w:caps" => p.all_caps = Some(child.is_on()),
            "w:smallCaps" => p.small_caps = Some(child.is_on()),
            "w:strike" => p.strike = Some(child.is_on()),
            "w:color" => p.color = child.value().and_then(parse_color),
            "w:sz" => p.size = num::<u16>(child, "w:val", log).map(FontSize),
            "w:highlight" => p.highlight = child.value().and_then(highlight_color),
            "w:u" => {
                p.underline = child
                    .value()
                    .and_then(underline_from)
                    .map(|style| Underline {
                        style,
                        color: child.get("w:color").and_then(parse_color),
                    });
            }
            "w:shd" => p.highlight = read_shading(child, log).map(|s| s.fill),
            "w:vertAlign" => p.vertical_align = child.value().and_then(vertical_align_from),
            other => log.unknown(&format!("{}/{other}", node.name())),
        }
    }
    p
}

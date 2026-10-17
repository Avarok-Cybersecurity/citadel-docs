//! `fo:border-*` and `fo:padding-*`: one edge is `"<width> <style> [<colour>]"` plus the
//! padding on that side, which is the model's border spacing. ODF has XSL's styles only, so
//! thick, dot-dash and triple lines are written as the nearest one (the writer's residual
//! keeps the exact style), and there is no edge between paragraphs.

use super::write_color;
use crate::units;
use cdoc_model::{Border, BorderStyle, BorderWidth, Borders, Color, Length};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

const EDGES: [&str; 4] = ["top", "bottom", "left", "right"];

/// Attributes this module reads.
pub const KEYS: [&str; 10] = [
    "fo:border",
    "fo:border-top",
    "fo:border-bottom",
    "fo:border-left",
    "fo:border-right",
    "fo:padding",
    "fo:padding-top",
    "fo:padding-bottom",
    "fo:padding-left",
    "fo:padding-right",
];

fn edge_of<'a>(borders: &'a Borders, edge: &str) -> Option<&'a Border> {
    match edge {
        "top" => borders.top.as_ref(),
        "bottom" => borders.bottom.as_ref(),
        "left" => borders.left.as_ref(),
        _ => borders.right.as_ref(),
    }
}

fn slot<'a>(borders: &'a mut Borders, edge: &str) -> &'a mut Option<Border> {
    match edge {
        "top" => &mut borders.top,
        "bottom" => &mut borders.bottom,
        "left" => &mut borders.left,
        _ => &mut borders.right,
    }
}

fn style_name(style: BorderStyle) -> &'static str {
    match style {
        BorderStyle::None => "none",
        BorderStyle::Single | BorderStyle::Thick => "solid",
        BorderStyle::Double | BorderStyle::Triple => "double",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Dashed | BorderStyle::DotDash => "dashed",
    }
}

/// Add the edges and their padding to a property element.
#[must_use]
pub fn write_borders(mut node: Node, borders: &Borders) -> Node {
    for edge in EDGES {
        if let Some(b) = edge_of(borders, edge) {
            let mut value = format!(
                "{} {}",
                units::write_eighths(b.width.0),
                style_name(b.style)
            );
            if let Some(c) = b.color {
                value.push(' ');
                value.push_str(&write_color(c));
            }
            node = node.attr(&format!("fo:border-{edge}"), &value).attr(
                &format!("fo:padding-{edge}"),
                &units::write_length(b.space.0),
            );
        }
    }
    node
}

fn read_style(token: &str, log: &mut Log) -> Option<BorderStyle> {
    Some(match token {
        "none" | "hidden" => BorderStyle::None,
        "solid" => BorderStyle::Single,
        "double" => BorderStyle::Double,
        "dotted" => BorderStyle::Dotted,
        "dashed" => BorderStyle::Dashed,
        "groove" | "ridge" | "inset" | "outset" => {
            log.lose(&format!("border style {token} read as solid"));
            BorderStyle::Single
        }
        _ => return None,
    })
}

fn read_edge(value: &str, space: Length, log: &mut Log) -> Option<Border> {
    let mut border = Border {
        style: BorderStyle::None,
        width: BorderWidth(0),
        color: None,
        space,
    };
    let mut styled = false;
    for token in value.split_whitespace() {
        if let Some(w) = units::eighths(token) {
            border.width = BorderWidth(w);
        } else if let Ok(c) = Color::parse_hex(token) {
            border.color = Some(c);
        } else if let Some(style) = read_style(token, log) {
            border.style = style;
            styled = true;
        } else {
            log.lose(&format!("unreadable border {value:?}"));
            return None;
        }
    }
    styled.then_some(border)
}

/// The edges a property element sets, `None` when it sets none. `get` looks attributes up.
pub fn read_borders<'a>(get: &dyn Fn(&str) -> Option<&'a str>, log: &mut Log) -> Option<Borders> {
    let mut borders = Borders::none();
    let mut any = false;
    for edge in EDGES {
        let value = get(&format!("fo:border-{edge}")).or_else(|| get("fo:border"));
        let padding = get(&format!("fo:padding-{edge}")).or_else(|| get("fo:padding"));
        let space = padding.and_then(units::length).map_or(Length(0), Length);
        if let Some(value) = value {
            *slot(&mut borders, edge) = read_edge(value, space, log);
            any = true;
        } else if padding.is_some() {
            log.lose("padding without a border");
        }
    }
    any.then_some(borders)
}

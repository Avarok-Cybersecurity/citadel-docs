//! Borders and shading.

use super::{border_style_from, border_style_name, num};
use crate::report::Log;
use crate::xml::Node;
use cdoc_model::{Border, BorderStyle, BorderWidth, Borders, Color, Length, Shading};

/// What the model's `between` edge is called in the container being written or read:
/// paragraphs have `between`, tables and cells have `insideH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BetweenEdge {
    /// `w:between`.
    Paragraph,
    /// `w:insideH`.
    Table,
}

impl BetweenEdge {
    const fn name(self) -> &'static str {
        match self {
            Self::Paragraph => "w:between",
            Self::Table => "w:insideH",
        }
    }
}

fn write_border(name: &str, border: &Border, log: &mut Log) -> Node {
    let points = super::round_div(border.space.0, Length::PER_POINT);
    if border.space.0 % Length::PER_POINT != 0 {
        log.lose("border spacing rounded to whole points");
    }
    Node::new(name)
        .attr("w:val", border_style_name(border.style))
        .num("w:sz", border.width.0)
        .num("w:space", points)
        .attr(
            "w:color",
            &border.color.map_or_else(|| "auto".to_owned(), hex6),
        )
}

pub(super) fn hex6(color: Color) -> String {
    format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

pub(super) fn parse_color(text: &str) -> Option<Color> {
    Color::parse_hex(text).ok()
}

fn read_border(node: &Node, log: &mut Log) -> Border {
    let style = match node.value() {
        Some(v) => border_style_from(v).unwrap_or_else(|| {
            log.lose("border style approximated as single");
            BorderStyle::Single
        }),
        None => BorderStyle::None,
    };
    Border {
        style,
        width: BorderWidth(num(node, "w:sz", log).unwrap_or(0)),
        color: node.get("w:color").and_then(parse_color),
        space: Length::from_points(num(node, "w:space", log).unwrap_or(0)),
    }
}

/// A border container: the four edges and `between`, in schema order.
#[must_use]
pub fn write_borders(name: &str, borders: &Borders, between: BetweenEdge, log: &mut Log) -> Node {
    let edges = [
        ("w:top", &borders.top),
        ("w:left", &borders.left),
        ("w:bottom", &borders.bottom),
        ("w:right", &borders.right),
        (between.name(), &borders.between),
    ];
    edges
        .into_iter()
        .filter_map(|(n, b)| b.as_ref().map(|b| write_border(n, b, log)))
        .fold(Node::new(name), Node::child)
}

/// Read a border container.
pub fn read_borders(node: &Node, between: BetweenEdge, log: &mut Log) -> Borders {
    let mut borders = Borders::none();
    for edge in node.elems() {
        let slot = match edge.name.as_str() {
            "w:top" => &mut borders.top,
            "w:left" | "w:start" => &mut borders.left,
            "w:bottom" => &mut borders.bottom,
            "w:right" | "w:end" => &mut borders.right,
            n if n == between.name() => &mut borders.between,
            other => {
                log.unknown(&format!("{}/{other}", node.name));
                continue;
            }
        };
        *slot = Some(read_border(edge, log));
    }
    borders
}

/// `<w:shd w:val="clear" w:fill=".."/>`.
#[must_use]
pub fn write_shading(shading: &Shading) -> Node {
    Node::new("w:shd")
        .attr("w:val", "clear")
        .attr("w:color", "auto")
        .attr("w:fill", &hex6(shading.fill))
}

/// Read shading; `None` when there is no fill to take (automatic).
pub fn read_shading(node: &Node, log: &mut Log) -> Option<Shading> {
    let pattern = node.value().unwrap_or("clear");
    let source = if pattern == "solid" {
        "w:color"
    } else {
        "w:fill"
    };
    if !matches!(pattern, "clear" | "solid" | "nil") {
        log.lose("patterned shading approximated by its fill");
    }
    node.get(source)
        .and_then(parse_color)
        .map(|fill| Shading { fill })
}

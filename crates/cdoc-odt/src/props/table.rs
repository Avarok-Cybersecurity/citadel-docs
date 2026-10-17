//! The table families' property elements: table, column, row and cell. ODF has no table-level
//! borders or cell padding and no cell widths (columns have widths); those stay in the
//! writer's residual.

use super::{border, color_attr, keep, read_keep, report_children, write_color};
use crate::units;
use cdoc_model::{
    Alignment, CellProperties, Length, RowHeight, Shading, TableProperties, TableRow, TableWidth,
    VerticalCellAlign,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

/// Element names.
pub const TABLE_PROPERTIES: &str = "style:table-properties";
/// Column properties.
pub const COLUMN_PROPERTIES: &str = "style:table-column-properties";
/// Row properties.
pub const ROW_PROPERTIES: &str = "style:table-row-properties";
/// Cell properties.
pub const CELL_PROPERTIES: &str = "style:table-cell-properties";

fn report_others(node: &Node, mapped: &[&str], log: &mut Log) {
    for (key, _) in &node.attrs {
        if !mapped.contains(&key.as_str()) {
            log.lose(&format!("{}@{key}", node.name));
        }
    }
    report_children(node, log);
}

fn unreadable(node: &Node, key: &str, log: &mut Log) {
    log.lose(&format!("unreadable value of {}@{key}", node.name));
}

/// Width and alignment; the style is the auto style's parent.
#[must_use]
pub fn write_table(p: &TableProperties) -> Node {
    let mut node = Node::new(TABLE_PROPERTIES);
    match p.width {
        TableWidth::Auto => {}
        TableWidth::Fixed { length } => {
            node = node.attr("style:width", &units::write_length(length.0));
        }
        TableWidth::Percent { fiftieths } => {
            node = node.attr("style:rel-width", &units::write_fiftieths(fiftieths));
        }
    }
    if let Some(a) = p.alignment {
        let align = match a {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justify => "margins",
        };
        node = node.attr("table:align", align);
    }
    node
}

/// Read table properties (the caller maps `known` attributes itself).
pub fn read_table(node: &Node, known: &[&str], log: &mut Log) -> TableProperties {
    let mut width = TableWidth::Auto;
    if let Some(v) = node.get("style:width") {
        match units::length(v) {
            Some(l) => width = TableWidth::Fixed { length: Length(l) },
            None => unreadable(node, "style:width", log),
        }
    }
    if let Some(v) = node.get("style:rel-width") {
        match units::fiftieths(v) {
            Some(fiftieths) => width = TableWidth::Percent { fiftieths },
            None => unreadable(node, "style:rel-width", log),
        }
    }
    let alignment = node.get("table:align").and_then(|v| match v {
        "left" => Some(Alignment::Left),
        "center" => Some(Alignment::Center),
        "right" => Some(Alignment::Right),
        "margins" => Some(Alignment::Justify),
        _ => {
            unreadable(node, "table:align", log);
            None
        }
    });
    let mut mapped = vec!["style:width", "style:rel-width", "table:align"];
    mapped.extend_from_slice(known);
    report_others(node, &mapped, log);
    TableProperties {
        style: None,
        width,
        alignment,
        borders: None,
        cell_padding: None,
    }
}

/// A column of this width.
#[must_use]
pub fn write_column(width: Length) -> Node {
    Node::new(COLUMN_PROPERTIES).attr("style:column-width", &units::write_length(width.0))
}

/// A column's width, if it states one.
pub fn read_column_width(node: &Node, log: &mut Log) -> Option<Length> {
    let width = node
        .get("style:column-width")
        .and_then(units::length)
        .map(Length);
    report_others(node, &["style:column-width", "style:rel-column-width"], log);
    width
}

/// Height rule and the no-split flag.
#[must_use]
pub fn write_row(row: &TableRow) -> Node {
    let node = Node::new(ROW_PROPERTIES);
    let node = match row.height {
        Some(RowHeight::Exact { height }) => {
            node.attr("style:row-height", &units::write_length(height.0))
        }
        Some(RowHeight::AtLeast { height }) => {
            node.attr("style:min-row-height", &units::write_length(height.0))
        }
        None => node,
    };
    node.attr(
        "fo:keep-together",
        keep(Some(row.cant_split)).unwrap_or("auto"),
    )
}

/// Read a row's height rule and no-split flag.
pub fn read_row(node: &Node, log: &mut Log) -> (Option<RowHeight>, bool) {
    let length = |key: &str| node.get(key).and_then(units::length).map(Length);
    let height = match (length("style:row-height"), length("style:min-row-height")) {
        (Some(height), _) => Some(RowHeight::Exact { height }),
        (None, Some(height)) => Some(RowHeight::AtLeast { height }),
        (None, None) => None,
    };
    let cant_split = node
        .get("fo:keep-together")
        .and_then(read_keep)
        .unwrap_or(false);
    report_others(
        node,
        &[
            "style:row-height",
            "style:min-row-height",
            "fo:keep-together",
        ],
        log,
    );
    (height, cant_split)
}

/// Borders, background and vertical alignment; spans live on the cell element.
#[must_use]
pub fn write_cell(p: &CellProperties) -> Node {
    let mut node = Node::new(CELL_PROPERTIES);
    if let Some(b) = &p.borders {
        node = border::write_borders(node, b);
    }
    if let Some(s) = &p.shading {
        node = node.attr("fo:background-color", &write_color(s.fill));
    }
    if let Some(v) = p.vertical_align {
        let value = match v {
            VerticalCellAlign::Top => "top",
            VerticalCellAlign::Center => "middle",
            VerticalCellAlign::Bottom => "bottom",
        };
        node = node.attr("style:vertical-align", value);
    }
    node
}

/// Read cell properties, spans 1 (the caller reads them from the cell).
pub fn read_cell(node: &Node, log: &mut Log) -> CellProperties {
    let mut p = CellProperties::single();
    p.borders = border::read_borders(&|k| node.get(k), log);
    let mut fill = None;
    if let Some(v) = node.get("fo:background-color") {
        if v != "transparent" && !color_attr(v, &mut fill) {
            unreadable(node, "fo:background-color", log);
        }
    }
    p.shading = fill.map(|fill| Shading { fill });
    p.vertical_align = node.get("style:vertical-align").and_then(|v| match v {
        "top" => Some(VerticalCellAlign::Top),
        "middle" => Some(VerticalCellAlign::Center),
        "bottom" => Some(VerticalCellAlign::Bottom),
        "automatic" => None,
        _ => {
            unreadable(node, "style:vertical-align", log);
            None
        }
    });
    let mut mapped = vec!["fo:background-color", "style:vertical-align"];
    mapped.extend_from_slice(&border::KEYS);
    report_others(node, &mapped, log);
    p
}

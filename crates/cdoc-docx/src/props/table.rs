//! Table, row and cell properties: `w:tblPr`, `w:trPr`, `w:tcPr`.

use super::{
    alignment_from, alignment_name, cell_align_from, cell_align_name, length, num, read_borders,
    read_shading, read_width, twips, write_borders, write_shading, write_width, BetweenEdge,
};
use crate::report::Log;
use crate::xml::Node;
use cdoc_model::{CellProperties, RowHeight, StyleId, TableProperties, TableRow, TableWidth};

const SIDES: [&str; 4] = ["w:top", "w:left", "w:bottom", "w:right"];

/// `w:tblPr`, children in schema order.
#[must_use]
pub fn write_table_properties(props: &TableProperties, log: &mut Log) -> Node {
    let mut node = Node::new("w:tblPr");
    if let Some(style) = &props.style {
        node = node.child(Node::val("w:tblStyle", style.as_str()));
    }
    node = node.child(write_width("w:tblW", props.width, log));
    if let Some(a) = props.alignment {
        node = node.child(Node::val("w:jc", alignment_name(a)));
    }
    if let Some(borders) = &props.borders {
        node = node.child(write_borders(
            "w:tblBorders",
            borders,
            BetweenEdge::Table,
            log,
        ));
    }
    if let Some(pad) = props.cell_padding {
        let w = twips(pad, log);
        let margins = SIDES
            .iter()
            .map(|side| Node::new(side).num("w:w", w).attr("w:type", "dxa"));
        node = node.child(Node::new("w:tblCellMar").children(margins));
    }
    node
}

/// Read `w:tblPr`.
pub fn read_table_properties(node: &Node, log: &mut Log) -> TableProperties {
    let mut p = TableProperties {
        style: None,
        width: TableWidth::Auto,
        alignment: None,
        borders: None,
        cell_padding: None,
    };
    for child in node.elems() {
        match child.name.as_str() {
            "w:tblStyle" => p.style = child.value().map(StyleId::new),
            "w:tblW" => p.width = read_width(child, log),
            "w:jc" => p.alignment = child.value().and_then(alignment_from),
            "w:tblBorders" => p.borders = Some(read_borders(child, BetweenEdge::Table, log)),
            "w:tblCellMar" => {
                let side = child.find("w:left").or_else(|| child.find("w:start"));
                p.cell_padding = side.and_then(|s| length(s, "w:w", log));
            }
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    p
}

/// `w:tcPr`; `vmerge` is `Some(true)` on the first cell of a vertical merge, `Some(false)` on
/// a continuation. Children in schema order.
#[must_use]
pub fn write_cell_properties(
    props: &CellProperties,
    header: bool,
    vmerge: Option<bool>,
    log: &mut Log,
) -> Node {
    let mut node = Node::new("w:tcPr");
    if header {
        node = node.child(Node::new("w:cnfStyle").attr("w:firstRow", "1"));
    }
    if let Some(width) = props.width {
        node = node.child(write_width("w:tcW", width, log));
    }
    if props.col_span != 1 {
        node = node.child(Node::val("w:gridSpan", &props.col_span.to_string()));
    }
    match vmerge {
        Some(true) => node = node.child(Node::val("w:vMerge", "restart")),
        Some(false) => node = node.child(Node::new("w:vMerge")),
        None => {}
    }
    if let Some(borders) = &props.borders {
        node = node.child(write_borders(
            "w:tcBorders",
            borders,
            BetweenEdge::Table,
            log,
        ));
    }
    if let Some(shading) = &props.shading {
        node = node.child(write_shading(shading));
    }
    if let Some(v) = props.vertical_align {
        node = node.child(Node::val("w:vAlign", cell_align_name(v)));
    }
    node
}

/// What `w:tcPr` says about a cell. `row_span` is 1 here: the table reader works it out from
/// the vertical merges.
pub fn read_cell_properties(node: &Node, log: &mut Log) -> (CellProperties, bool, Option<bool>) {
    let mut p = CellProperties::single();
    let mut header = false;
    let mut vmerge = None;
    for child in node.elems() {
        match child.name.as_str() {
            "w:cnfStyle" => header = child.get("w:firstRow") == Some("1"),
            "w:tcW" => p.width = Some(read_width(child, log)),
            "w:gridSpan" => p.col_span = num(child, "w:val", log).filter(|n| *n > 0).unwrap_or(1),
            "w:vMerge" => vmerge = Some(child.value() == Some("restart")),
            "w:tcBorders" => p.borders = Some(read_borders(child, BetweenEdge::Table, log)),
            "w:shd" => p.shading = read_shading(child, log),
            "w:vAlign" => p.vertical_align = child.value().and_then(cell_align_from),
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    (p, header, vmerge)
}

/// `w:trPr` for a row; `repeat_header` marks a header row that repeats across pages.
#[must_use]
pub fn write_row_properties(row: &TableRow, repeat_header: bool, log: &mut Log) -> Node {
    let mut node = Node::new("w:trPr");
    if row.cant_split {
        node = node.child(Node::new("w:cantSplit"));
    }
    if let Some(h) = row.height {
        let (height, rule) = match h {
            RowHeight::AtLeast { height } => (height, "atLeast"),
            RowHeight::Exact { height } => (height, "exact"),
        };
        node = node.child(
            Node::new("w:trHeight")
                .num("w:val", twips(height, log))
                .attr("w:hRule", rule),
        );
    }
    if repeat_header {
        node = node.child(Node::new("w:tblHeader"));
    }
    node
}

/// Row-level facts from `w:trPr`: height, cant-split, repeated header.
pub fn read_row_flags(node: &Node, log: &mut Log) -> (Option<RowHeight>, bool, bool) {
    let (mut height, mut cant_split, mut header) = (None, false, false);
    for child in node.elems() {
        match child.name.as_str() {
            "w:cantSplit" => cant_split = child.is_on(),
            "w:tblHeader" => header = child.is_on(),
            "w:trHeight" => {
                height = length(child, "w:val", log).map(|height| match child.get("w:hRule") {
                    Some("exact") => RowHeight::Exact { height },
                    _ => RowHeight::AtLeast { height },
                });
            }
            other => log.unknown(&format!("{}/{other}", node.name)),
        }
    }
    (height, cant_split, header)
}

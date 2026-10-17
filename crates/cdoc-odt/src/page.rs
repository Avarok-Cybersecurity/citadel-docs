//! `style:page-layout` and [`PageSetup`]. ODF measures the top margin to the header and gives
//! the header its own height; Word (and the model) measure the top margin to the body and the
//! header distance from the edge. With a header the layout's top margin is the header distance
//! and the header's minimum height fills the gap to the body, so both readings agree; the same
//! holds at the bottom. Without a header the layout's margin is the body margin and the header
//! distance has no slot (it is read as half the margin). The gutter has no ODF 1.2 slot.

use crate::props::write_color;
use crate::units;
use cdoc_model::{
    Columns, Length, Margins, NumberFormat, Orientation, PageNumbering, PageSetup, PageSize,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

const FORMATS: [(NumberFormat, &str); 6] = [
    (NumberFormat::Decimal, "1"),
    (NumberFormat::LowerLetter, "a"),
    (NumberFormat::UpperLetter, "A"),
    (NumberFormat::LowerRoman, "i"),
    (NumberFormat::UpperRoman, "I"),
    (NumberFormat::None, ""),
];

fn len(l: Length) -> String {
    units::write_length(l.0)
}

fn band(name: &str, gap_key: &str, min_height: Length) -> Node {
    Node::new(name).child(
        Node::new("style:header-footer-properties")
            .attr("fo:min-height", &len(Length(min_height.0.max(0))))
            .attr(gap_key, "0pt"),
    )
}

/// The layout for a page setup; `header` / `footer` say whether the master pages using it have
/// one.
#[must_use]
pub fn write_layout(name: &str, page: &PageSetup, header: bool, footer: bool) -> Node {
    let m = page.margins;
    let format = FORMATS
        .iter()
        .find(|(f, _)| *f == page.page_numbering.format)
        .map_or("1", |(_, s)| s);
    let orientation = match page.size.orientation {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    let mut columns = Node::new("style:columns")
        .num("fo:column-count", page.columns.count)
        .attr("fo:column-gap", &len(page.columns.gap));
    if page.columns.separator {
        columns = columns.child(
            Node::new("style:column-sep")
                .attr("style:width", "0.5pt")
                .attr("style:color", &write_color(cdoc_model::Color::rgb(0, 0, 0))),
        );
    }
    let properties = Node::new("style:page-layout-properties")
        .attr("fo:page-width", &len(page.size.width))
        .attr("fo:page-height", &len(page.size.height))
        .attr("style:print-orientation", orientation)
        .attr("style:num-format", format)
        .attr("fo:margin-top", &len(if header { m.header } else { m.top }))
        .attr(
            "fo:margin-bottom",
            &len(if footer { m.footer } else { m.bottom }),
        )
        .attr("fo:margin-left", &len(m.left))
        .attr("fo:margin-right", &len(m.right))
        .child(columns);
    let mut layout = Node::new("style:page-layout")
        .attr("style:name", name)
        .child(properties);
    if header {
        layout = layout.child(band(
            "style:header-style",
            "fo:margin-bottom",
            Length(m.top.0 - m.header.0),
        ));
    }
    if footer {
        layout = layout.child(band(
            "style:footer-style",
            "fo:margin-top",
            Length(m.bottom.0 - m.footer.0),
        ));
    }
    layout
}

fn band_height(layout: &Node, name: &str, gap_key: &str) -> Option<i32> {
    let props = layout.find(name)?.find("style:header-footer-properties")?;
    let height = props
        .get("fo:min-height")
        .or_else(|| props.get("svg:height"))
        .and_then(units::length)
        .unwrap_or(0);
    let gap = props.get(gap_key).and_then(units::length).unwrap_or(0);
    Some(height + gap)
}

fn columns(node: Option<&Node>) -> Columns {
    let Some(node) = node else {
        return Columns::ONE;
    };
    Columns {
        count: node
            .get("fo:column-count")
            .and_then(|c| c.parse().ok())
            .filter(|c| *c >= 1)
            .unwrap_or(1),
        gap: node
            .get("fo:column-gap")
            .and_then(units::length)
            .map_or(Columns::ONE.gap, Length),
        separator: node.find("style:column-sep").is_some(),
    }
}

const MAPPED: [&str; 9] = [
    "fo:page-width",
    "fo:page-height",
    "style:print-orientation",
    "style:num-format",
    "fo:margin-top",
    "fo:margin-bottom",
    "fo:margin-left",
    "fo:margin-right",
    "fo:margin",
];

/// Read a layout; `header` / `footer` say whether the section's master pages have one,
/// `start` is the page number the section's first block restarts at.
pub fn read_layout(
    layout: &Node,
    header: bool,
    footer: bool,
    start: Option<u32>,
    log: &mut Log,
) -> PageSetup {
    let empty = Node::new("style:page-layout-properties");
    let p = layout
        .find("style:page-layout-properties")
        .unwrap_or(&empty);
    let get = |key: &str| {
        p.get(key)
            .or_else(|| p.get("fo:margin").filter(|_| key.starts_with("fo:margin")))
            .and_then(units::length)
    };
    let (width, height) = match (get("fo:page-width"), get("fo:page-height")) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (Length(w), Length(h)),
        _ => {
            log.lose("a page layout without a size; A4 assumed");
            (PageSize::A4_PORTRAIT.width, PageSize::A4_PORTRAIT.height)
        }
    };
    let orientation = match p.get("style:print-orientation") {
        Some("landscape") => Orientation::Landscape,
        Some("portrait") => Orientation::Portrait,
        _ if width > height => Orientation::Landscape,
        _ => Orientation::Portrait,
    };
    let edge = |key: &str| Length(get(key).unwrap_or(0).max(0));
    let (outer_top, outer_bottom) = (edge("fo:margin-top"), edge("fo:margin-bottom"));
    let header_band =
        band_height(layout, "style:header-style", "fo:margin-bottom").filter(|_| header);
    let footer_band = band_height(layout, "style:footer-style", "fo:margin-top").filter(|_| footer);
    let (top, header_at) = match header_band {
        Some(h) => (Length(outer_top.0 + h), outer_top),
        None => (outer_top, Length(outer_top.0 / 2)),
    };
    let (bottom, footer_at) = match footer_band {
        Some(h) => (Length(outer_bottom.0 + h), outer_bottom),
        None => (outer_bottom, Length(outer_bottom.0 / 2)),
    };
    let raw = p.get("style:num-format").unwrap_or("1");
    let format = FORMATS.iter().find(|(_, s)| *s == raw).map_or_else(
        || {
            log.lose(&format!("page number format {raw:?} read as decimal"));
            NumberFormat::Decimal
        },
        |(f, _)| *f,
    );
    for (key, _) in &p.attrs {
        if !MAPPED.contains(&key.as_str()) {
            log.lose(&format!("{}@{key}", p.name));
        }
    }
    for child in p.elems().filter(|c| c.name != "style:columns") {
        log.lose(&format!("{}/{}", p.name, child.name));
    }
    if let Some(c) = p.find("style:columns") {
        let mut widths: Vec<&str> = c
            .elems()
            .filter(|e| e.name == "style:column")
            .filter_map(|e| e.get("style:rel-width"))
            .collect();
        widths.dedup();
        if widths.len() > 1 {
            log.lose("columns of unequal width; equal columns assumed");
        }
    }
    PageSetup {
        size: PageSize {
            width,
            height,
            orientation,
        },
        margins: Margins {
            top,
            bottom,
            left: edge("fo:margin-left"),
            right: edge("fo:margin-right"),
            header: header_at,
            footer: footer_at,
            gutter: Length(0),
        },
        columns: columns(p.find("style:columns")),
        page_numbering: PageNumbering { format, start },
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;

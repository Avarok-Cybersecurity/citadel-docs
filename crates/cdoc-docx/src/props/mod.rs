//! Property containers (`w:rPr`, `w:pPr`, borders, table and cell properties), each with its
//! writer and reader side by side, built from one table of names per enum.

mod border;
mod names;
mod para;
mod run;
mod table;

pub use border::{read_borders, read_shading, write_borders, write_shading, BetweenEdge};
pub use names::*;
pub use para::{read_indent, read_ppr, write_indent, write_ppr};
pub use run::{read_rpr, read_rpr_skipping, write_rpr};
pub use table::{
    read_cell_properties, read_row_flags, read_table_properties, write_cell_properties,
    write_row_properties, write_table_properties,
};

use crate::report::Log;
use crate::xml::{El, Node};
use cdoc_model::{Length, TableWidth};
use std::str::FromStr;

/// A length as twips for writing, noting a loss when it is not a whole number of twips.
pub fn twips(length: Length, log: &mut Log) -> i32 {
    if length.0 % Length::PER_TWIP != 0 {
        log.lose("length rounded to whole twips");
    }
    round_div(length.0, Length::PER_TWIP)
}

/// Integer division rounding half up (toward positive infinity), exact for negatives.
#[must_use]
pub const fn round_div(n: i32, d: i32) -> i32 {
    (n + d / 2).div_euclid(d)
}

/// A numeric attribute; a present but unparseable value is noted and treated as absent.
pub fn num<T: FromStr>(node: El<'_>, key: &str, log: &mut Log) -> Option<T> {
    let raw = node.get(key)?;
    let parsed = raw.trim().parse().ok();
    if parsed.is_none() {
        log.lose(&format!("unparseable value of {} on {}", key, node.name()));
    }
    parsed
}

/// A twips attribute as a [`Length`].
pub fn length(node: El<'_>, key: &str, log: &mut Log) -> Option<Length> {
    num::<i32>(node, key, log).map(Length::from_twips)
}

/// `<name w:w=".." w:type=".."/>` for a table or cell width.
pub fn write_width(name: &str, width: TableWidth, log: &mut Log) -> Node {
    let node = Node::new(name);
    match width {
        TableWidth::Auto => node.attr("w:w", "0").attr("w:type", "auto"),
        TableWidth::Fixed { length } => node.num("w:w", twips(length, log)).attr("w:type", "dxa"),
        TableWidth::Percent { fiftieths } => node.num("w:w", fiftieths).attr("w:type", "pct"),
    }
}

/// Read a width element. Percentages written as `"50%"` text are not modelled and noted.
pub fn read_width(node: El<'_>, log: &mut Log) -> TableWidth {
    let amount = num::<i32>(node, "w:w", log);
    match (node.get("w:type"), amount) {
        (Some("dxa"), Some(w)) => TableWidth::Fixed {
            length: Length::from_twips(w),
        },
        (Some("pct"), Some(w)) => u16::try_from(w).map_or_else(
            |_| {
                log.lose("percentage width out of range");
                TableWidth::Auto
            },
            |fiftieths| TableWidth::Percent { fiftieths },
        ),
        (Some("auto" | "nil"), _) | (None, None) => TableWidth::Auto,
        (_, w) => {
            log.lose("width of an unsupported type");
            w.map_or(TableWidth::Auto, |w| TableWidth::Fixed {
                length: Length::from_twips(w),
            })
        }
    }
}

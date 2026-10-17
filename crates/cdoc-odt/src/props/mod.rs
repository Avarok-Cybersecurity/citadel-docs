//! Property elements (`style:text-properties`, `style:paragraph-properties`, the table
//! families') and the model's property structs, each writer beside its reader. The writers
//! emit only what the model sets; the readers report every attribute and child they do not
//! map.

mod border;
mod para;
mod run;
mod table;

pub use para::{read_paragraph, write_paragraph, PARAGRAPH_PROPERTIES};
pub use run::{read_run, write_run, TEXT_PROPERTIES};
pub use table::{
    read_cell, read_column_width, read_row, read_table, write_cell, write_column, write_row,
    write_table, CELL_PROPERTIES, COLUMN_PROPERTIES, ROW_PROPERTIES, TABLE_PROPERTIES,
};

use cdoc_model::Color;
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

/// `#rrggbb`, the spelling ODF producers use.
#[must_use]
pub fn write_color(c: Color) -> String {
    c.hex().to_ascii_lowercase()
}

/// Parse a colour attribute into `slot`; whether it parsed.
pub fn color_attr(value: &str, slot: &mut Option<Color>) -> bool {
    match Color::parse_hex(value) {
        Ok(c) => {
            *slot = Some(c);
            true
        }
        Err(_) => false,
    }
}

/// A font face's `svg:font-family` as this crate writes it: the name, quoted.
#[must_use]
pub fn quote_family(name: &str) -> String {
    if name.contains('\'') {
        format!("\"{name}\"")
    } else {
        format!("'{name}'")
    }
}

/// The family a font face declares: its name when the declaration is this crate's own quoting
/// of it (so any name survives), else the first family of the list.
#[must_use]
pub fn face_family(name: &str, declared: Option<&str>) -> String {
    match declared {
        Some(d) if d == quote_family(name) => name.to_owned(),
        Some(d) => first_family(d).unwrap_or_else(|| name.to_owned()),
        None => name.to_owned(),
    }
}

/// The first family of a `fo:font-family` list, unquoted.
#[must_use]
pub fn first_family(value: &str) -> Option<String> {
    let first = value.split(',').next()?.trim();
    let unquoted = first.trim_matches(|c| c == '\'' || c == '"').trim();
    (!unquoted.is_empty()).then(|| unquoted.to_owned())
}

/// Report every child element of a property element: none is mapped unless the caller says.
pub fn report_children(node: &Node, log: &mut Log) {
    for child in node.elems() {
        log.lose(&format!("{}/{}", node.name, child.name));
    }
}

/// `always` / `auto` for a keep rule.
fn keep(value: Option<bool>) -> Option<&'static str> {
    value.map(|v| if v { "always" } else { "auto" })
}

/// Read a keep rule.
fn read_keep(value: &str) -> Option<bool> {
    match value {
        "always" => Some(true),
        "auto" => Some(false),
        _ => None,
    }
}

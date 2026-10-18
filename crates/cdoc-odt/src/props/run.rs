//! `style:text-properties` and [`RunProperties`].

use super::{color_attr, report_children, write_color};
use crate::units;
use cdoc_model::{Color, FontSize, RunProperties, Underline, UnderlineStyle, VerticalAlign};
use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;
use std::collections::BTreeMap;

/// The element name.
pub const TEXT_PROPERTIES: &str = "style:text-properties";

fn underline_attrs(node: Node, u: &Underline) -> Node {
    let (style, kind, width, mode) = match u.style {
        UnderlineStyle::None => ("none", None, None, None),
        UnderlineStyle::Single => ("solid", None, None, None),
        UnderlineStyle::Double => ("solid", Some("double"), None, None),
        UnderlineStyle::Thick => ("solid", None, Some("bold"), None),
        UnderlineStyle::Dotted => ("dotted", None, None, None),
        UnderlineStyle::Dashed => ("dash", None, None, None),
        UnderlineStyle::Wave => ("wave", None, None, None),
        UnderlineStyle::Words => ("solid", None, None, Some("skip-white-space")),
    };
    let mut node = node.attr("style:text-underline-style", style);
    if let Some(kind) = kind {
        node = node.attr("style:text-underline-type", kind);
    }
    if let Some(width) = width {
        node = node.attr("style:text-underline-width", width);
    }
    if let Some(mode) = mode {
        node = node.attr("style:text-underline-mode", mode);
    }
    let color = u.color.map_or_else(|| "font-color".to_owned(), write_color);
    node.attr("style:text-underline-color", &color)
}

fn on(node: Node, key: &str, value: Option<bool>, yes: &str, no: &str) -> Node {
    match value {
        Some(v) => node.attr(key, if v { yes } else { no }),
        None => node,
    }
}

/// The element for direct run formatting (the character style, if any, is the parent).
#[must_use]
pub fn write_run(props: &RunProperties) -> Node {
    let mut node = Node::new(TEXT_PROPERTIES);
    if let Some(font) = &props.font {
        node = node.attr("style:font-name", font);
    }
    if let Some(size) = props.size {
        node = node.attr("fo:font-size", &units::write_half_points(size.0));
    }
    node = on(node, "fo:font-weight", props.bold, "bold", "normal");
    node = on(node, "fo:font-style", props.italic, "italic", "normal");
    if let Some(u) = &props.underline {
        node = underline_attrs(node, u);
    }
    node = on(
        node,
        "style:text-line-through-style",
        props.strike,
        "solid",
        "none",
    );
    if let Some(v) = props.vertical_align {
        let position = match v {
            VerticalAlign::Baseline => "0% 100%",
            VerticalAlign::Superscript => "super 58%",
            VerticalAlign::Subscript => "sub 58%",
        };
        node = node.attr("style:text-position", position);
    }
    if let Some(c) = props.color {
        node = node.attr("fo:color", &write_color(c));
    }
    if let Some(c) = props.highlight {
        node = node.attr("fo:background-color", &write_color(c));
    }
    node = on(
        node,
        "fo:font-variant",
        props.small_caps,
        "small-caps",
        "normal",
    );
    on(
        node,
        "fo:text-transform",
        props.all_caps,
        "uppercase",
        "none",
    )
}

fn underline_style(attrs: &BTreeMap<&str, &str>, log: &mut Log) -> Option<UnderlineStyle> {
    let style = attrs.get("style:text-underline-style")?;
    let double = attrs.get("style:text-underline-type") == Some(&"double");
    let bold = attrs.get("style:text-underline-width") == Some(&"bold");
    let words = attrs.get("style:text-underline-mode") == Some(&"skip-white-space");
    Some(match *style {
        "none" => UnderlineStyle::None,
        _ if double => UnderlineStyle::Double,
        "solid" if bold => UnderlineStyle::Thick,
        "solid" if words => UnderlineStyle::Words,
        "solid" => UnderlineStyle::Single,
        "dotted" => UnderlineStyle::Dotted,
        "dash" => UnderlineStyle::Dashed,
        "wave" => UnderlineStyle::Wave,
        "long-dash" | "dot-dash" | "dot-dot-dash" => {
            log.lose(&format!("underline style {style} read as dashed"));
            UnderlineStyle::Dashed
        }
        other => {
            log.lose(&format!("underline style {other} read as single"));
            UnderlineStyle::Single
        }
    })
}

fn yes_no(value: &str, yes: &[&str], no: &[&str]) -> Option<bool> {
    if yes.contains(&value) {
        Some(true)
    } else if no.contains(&value) {
        Some(false)
    } else {
        None
    }
}

const BOLD: [&str; 5] = ["bold", "600", "700", "800", "900"];
const NOT_BOLD: [&str; 6] = ["normal", "100", "200", "300", "400", "500"];

/// Read the element; `faces` maps `style:font-name` values to family names (a name with no
/// declaration is its own family).
pub fn read_run(node: &Node, faces: &BTreeMap<String, String>, log: &mut Log) -> RunProperties {
    let attrs: BTreeMap<&str, &str> = node
        .attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut p = RunProperties::none();
    for (&key, &value) in &attrs {
        let parsed = match key {
            "style:font-name" => {
                p.font = Some(
                    faces
                        .get(value)
                        .cloned()
                        .unwrap_or_else(|| value.to_owned()),
                );
                true
            }
            "fo:font-family" => {
                if p.font.is_none() {
                    p.font = super::first_family(value);
                }
                true
            }
            "fo:font-size" => {
                p.size = units::half_points(value).filter(|s| *s > 0).map(FontSize);
                p.size.is_some()
            }
            "fo:font-weight" => {
                p.bold = yes_no(value, &BOLD, &NOT_BOLD);
                p.bold.is_some()
            }
            "fo:font-style" => {
                p.italic = yes_no(value, &["italic", "oblique"], &["normal"]);
                p.italic.is_some()
            }
            "style:text-line-through-style" => {
                p.strike = Some(value != "none");
                true
            }
            "style:text-position" => {
                p.vertical_align = vertical(value);
                p.vertical_align.is_some()
            }
            "fo:color" => color_attr(value, &mut p.color),
            "fo:background-color" => value == "transparent" || color_attr(value, &mut p.highlight),
            "fo:font-variant" => {
                p.small_caps = yes_no(value, &["small-caps"], &["normal"]);
                p.small_caps.is_some()
            }
            "fo:text-transform" => {
                p.all_caps = yes_no(value, &["uppercase"], &["none"]);
                p.all_caps.is_some()
            }
            "style:text-underline-style"
            | "style:text-underline-type"
            | "style:text-underline-width"
            | "style:text-underline-mode" => true,
            "style:text-underline-color" => {
                value == "font-color" || Color::parse_hex(value).is_ok()
            }
            other => {
                log.lose(&format!("{TEXT_PROPERTIES}@{other}"));
                true
            }
        };
        if !parsed {
            log.lose(&format!("unreadable value of {TEXT_PROPERTIES}@{key}"));
        }
    }
    if let Some(style) = underline_style(&attrs, log) {
        let color = attrs
            .get("style:text-underline-color")
            .and_then(|c| Color::parse_hex(c).ok());
        p.underline = Some(Underline { style, color });
    }
    report_children(node, log);
    p
}

fn vertical(value: &str) -> Option<VerticalAlign> {
    let first = value.split_whitespace().next()?;
    match first {
        "super" => Some(VerticalAlign::Superscript),
        "sub" => Some(VerticalAlign::Subscript),
        pct => {
            let n = units::percent(pct, 1, 1)?;
            Some(match n {
                0 => VerticalAlign::Baseline,
                n if n > 0 => VerticalAlign::Superscript,
                _ => VerticalAlign::Subscript,
            })
        }
    }
}

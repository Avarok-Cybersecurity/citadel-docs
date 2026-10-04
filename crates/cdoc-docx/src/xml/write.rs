//! A [`Node`] tree to XML text.

use super::{Child, Node};
use std::fmt::Write;

/// Serialise with an XML declaration. Characters XML 1.0 cannot carry are replaced with U+FFFD;
/// the second value counts them so the caller can report the substitution.
#[must_use]
pub fn serialize(root: &Node) -> (Vec<u8>, usize) {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let mut replaced = 0;
    element(root, &mut out, &mut replaced);
    (out.into_bytes(), replaced)
}

fn element(node: &Node, out: &mut String, replaced: &mut usize) {
    out.push('<');
    out.push_str(&node.name);
    for (key, value) in &node.attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        escape(value, true, out, replaced);
        out.push('"');
    }
    if node.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &node.children {
        match child {
            Child::Node(n) => element(n, out, replaced),
            Child::Text(t) => escape(t, false, out, replaced),
        }
    }
    out.push_str("</");
    out.push_str(&node.name);
    out.push('>');
}

fn allowed(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

fn escape(text: &str, attribute: bool, out: &mut String, replaced: &mut usize) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            '\t' | '\n' | '\r' if attribute => {
                let _ = write!(out, "&#{};", u32::from(c));
            }
            '\r' => out.push_str("&#13;"),
            c if allowed(c) => out.push(c),
            _ => {
                *replaced += 1;
                out.push('\u{FFFD}');
            }
        }
    }
}

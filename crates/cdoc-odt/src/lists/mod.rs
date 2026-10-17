//! `text:list-style` and [`AbstractNumbering`]. A level's label template (`%1.%2.`) becomes a
//! prefix, a run of consecutive levels joined by `.` (`text:display-levels`) and a suffix;
//! a bullet is its glyph. Templates ODF cannot spell, a start of zero and a right indent stay
//! in the writer's residual.

mod read;

pub use read::read_list_style;

use crate::props::write_run;
use crate::units;
use cdoc_model::{AbstractNumbering, Alignment, FirstLine, Indent, Level, NumberFormat};
use cdoc_pkg::xml::Node;

const FORMATS: [(NumberFormat, &str); 6] = [
    (NumberFormat::Decimal, "1"),
    (NumberFormat::LowerLetter, "a"),
    (NumberFormat::UpperLetter, "A"),
    (NumberFormat::LowerRoman, "i"),
    (NumberFormat::UpperRoman, "I"),
    (NumberFormat::None, ""),
];

/// `prefix`, levels shown, `suffix` for a template at a 1-based level, when ODF can spell it.
fn spell(template: &str, level: usize) -> Option<(String, usize, String)> {
    let start = template.find('%')?;
    let prefix = &template[..start];
    let mut rest = &template[start..];
    let mut levels = Vec::new();
    while let Some(after) = rest.strip_prefix('%') {
        let digit = after.chars().next()?.to_digit(10)?;
        levels.push(usize::try_from(digit).ok()?);
        rest = &after[1..];
        match rest.strip_prefix(".%") {
            Some(next) if next.starts_with(|c: char| c.is_ascii_digit()) => rest = &rest[1..],
            _ => break,
        }
    }
    let consecutive = levels.windows(2).all(|w| w[1] == w[0] + 1);
    let ends_here = levels.last() == Some(&level);
    (consecutive && ends_here && !rest.contains('%') && !prefix.contains('%'))
        .then(|| (prefix.to_owned(), levels.len(), rest.to_owned()))
}

/// The template a prefix, levels shown and suffix spell.
fn unspell(prefix: &str, shown: usize, suffix: &str, level: usize) -> String {
    let first = (level + 1).saturating_sub(shown.max(1)).max(1);
    let numbers: Vec<String> = (first..=level).map(|k| format!("%{k}")).collect();
    format!("{prefix}{}{suffix}", numbers.join("."))
}

fn indent_node(indent: Option<&Indent>) -> Node {
    let mut label =
        Node::new("style:list-level-label-alignment").attr("text:label-followed-by", "listtab");
    if let Some(i) = indent {
        if let Some(left) = i.left {
            let at = units::write_length(left.0);
            label = label
                .attr("text:list-tab-stop-position", &at)
                .attr("fo:margin-left", &at);
        }
        match i.first_line {
            Some(FirstLine::Indent { by }) => {
                label = label.attr("fo:text-indent", &units::write_signed_length(by.0, false));
            }
            Some(FirstLine::Hanging { by }) => {
                label = label.attr("fo:text-indent", &units::write_signed_length(by.0, true));
            }
            None => {}
        }
    }
    label
}

fn write_level(index: usize, level: &Level) -> Node {
    let number = index + 1;
    let mut node = if level.format == NumberFormat::Bullet {
        let glyph: String = level
            .text
            .chars()
            .next()
            .map_or_else(|| "\u{2022}".to_owned(), String::from);
        Node::new("text:list-level-style-bullet")
            .num("text:level", number)
            .attr("text:bullet-char", &glyph)
    } else {
        let format = FORMATS
            .iter()
            .find(|(f, _)| *f == level.format)
            .map_or("1", |(_, s)| s);
        let (prefix, shown, suffix) = spell(&level.text, number).unwrap_or_else(|| {
            let before = level.text.split('%').next().unwrap_or("").to_owned();
            (before, 1, String::new())
        });
        let mut n = Node::new("text:list-level-style-number").num("text:level", number);
        if !prefix.is_empty() {
            n = n.attr("style:num-prefix", &prefix);
        }
        if !suffix.is_empty() {
            n = n.attr("style:num-suffix", &suffix);
        }
        n = n.attr("style:num-format", format);
        if shown > 1 {
            n = n.num("text:display-levels", shown);
        }
        if level.start >= 1 {
            n = n.num("text:start-value", level.start);
        }
        n
    };
    let align = match level.alignment {
        Alignment::Left => "start",
        Alignment::Center => "center",
        Alignment::Right => "end",
        Alignment::Justify => "justify",
    };
    let properties = Node::new("style:list-level-properties")
        .attr("text:list-level-position-and-space-mode", "label-alignment")
        .attr("fo:text-align", align)
        .child(indent_node(level.indent.as_ref()));
    node = node.child(properties);
    if let Some(run) = &level.run {
        node = node.child(write_run(run));
    }
    node
}

/// The list style for a definition.
#[must_use]
pub fn write_list_style(name: &str, def: &AbstractNumbering) -> Node {
    Node::new("text:list-style")
        .attr("style:name", name)
        .children(
            def.levels
                .iter()
                .enumerate()
                .map(|(i, l)| write_level(i, l)),
        )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

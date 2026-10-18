//! Escaping text for Markdown. Every character that can start a construct is
//! backslash-escaped wherever it appears: CommonMark allows a backslash before any ASCII
//! punctuation, so over-escaping changes nothing on read and under-escaping would change
//! meaning.

/// Characters escaped everywhere.
const ALWAYS: &str = "\\`*_[]<>~|&!#()";

/// Escape `text`. `line_start` says the text begins a line of block content, where `-`, `+`
/// and `1.` would open a list and leading spaces would be lost or open a code block.
pub fn escape(text: &str, line_start: bool) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    let mut at_start = line_start;
    let mut digits = false;
    for c in text.chars() {
        let punctuation_after_digits = digits && matches!(c, '.' | ')');
        if ALWAYS.contains(c)
            || punctuation_after_digits
            || (at_start && matches!(c, '-' | '+' | '='))
        {
            out.push('\\');
            out.push(c);
        } else if at_start && (c == ' ' || c == '\t') {
            // Leading whitespace is settled per line once the line is whole
            // (`settle_whitespace`); what follows it still starts the line.
            out.push(c);
            continue;
        } else {
            out.push(c);
        }
        digits = (at_start || digits) && c.is_ascii_digit();
        at_start = false;
    }
    out
}

/// What [`settle_whitespace`] could not keep.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Dropped {
    /// A line began with a tab, or with four or more columns of spaces.
    pub leading: bool,
    /// The text ended with a tab.
    pub trailing: bool,
}

/// The whitespace at the start of each line and at the end of `text`, in forms Markdown keeps.
/// Up to three leading spaces are kept as `&#32;` (parsers strip them otherwise). A leading
/// run with a tab, or of four or more columns, would open an indented code block, so it
/// becomes one space, which reading drops: Markdown has no leading tab. A trailing space is
/// kept as `&#32;`; a trailing tab is dropped. No tab is ever written as an entity: tabs
/// inside a line are literal. `first_line_starts` says whether `text` itself begins a line.
pub fn settle_whitespace(text: &str, first_line_starts: bool) -> (String, Dropped) {
    let mut dropped = Dropped::default();
    let mut out = String::with_capacity(text.len());
    // Only a hard break (`\` at the end of a line) starts a line inside a paragraph; any
    // other newline is inside a construct whose content is not ours to change.
    let mut starts = first_line_starts;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let body = line.trim_start_matches([' ', '\t']);
        let lead = &line[..line.len() - body.len()];
        let at_line_start = starts;
        starts = line.ends_with('\\');
        if at_line_start && !lead.is_empty() {
            if lead.contains('\t') || lead.len() >= 4 {
                dropped.leading = true;
                out.push(' ');
            } else {
                out.push_str(&"&#32;".repeat(lead.len()));
            }
        } else {
            out.push_str(lead);
        }
        out.push_str(body);
    }
    match out.pop() {
        Some(' ') => out.push_str("&#32;"),
        Some('\t') => dropped.trailing = true,
        Some(c) => out.push(c),
        None => {}
    }
    (out, dropped)
}

/// A code span around `text`: a fence one backtick longer than any run inside, padded when the
/// content begins or ends with a backtick or a space.
pub fn code_span(text: &str) -> String {
    let longest = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest + 1);
    let pad = text.starts_with('`')
        || text.ends_with('`')
        || text.starts_with(' ')
        || text.ends_with(' ');
    if pad {
        format!("{fence} {text} {fence}")
    } else {
        format!("{fence}{text}{fence}")
    }
}

/// A link or image destination in angle brackets, with the characters that cannot appear
/// inside them percent-encoded.
pub fn destination(url: &str) -> String {
    let mut out = String::from("<");
    for c in url.chars() {
        match c {
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('>');
    out
}

/// A link title in double quotes.
pub fn title(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

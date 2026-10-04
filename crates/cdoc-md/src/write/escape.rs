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
            out.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
            continue;
        } else {
            out.push(c);
        }
        digits = (at_start || digits) && c.is_ascii_digit();
        at_start = false;
    }
    out
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

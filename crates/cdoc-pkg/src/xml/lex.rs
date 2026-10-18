//! The tokeniser: XML text, already checked to be UTF-8, straight into a [`Builder`]. Names,
//! attribute values and text are slices of the input unless an entity had to be resolved.
//!
//! It accepts what the package formats write: elements, attributes in either quote, the five
//! predefined entities and character references, CDATA, comments, processing instructions and
//! a document type declaration (skipped). It refuses unbalanced or mismatched tags, duplicate
//! attributes, an attribute without a quoted value and unknown entities, as a conforming parser
//! must; it does not check that names are made of name characters.

use super::scan::{find, find2};
use super::tree::{Builder, Shape};
use std::borrow::Cow;

/// Why the text is not XML the tokeniser accepts.
pub(super) type LexError = String;

impl From<Shape> for LexError {
    fn from(shape: Shape) -> Self {
        match shape {
            Shape::Unbalanced => "unbalanced end tag or content after the document element",
            Shape::TooMany => "too many nodes",
        }
        .to_owned()
    }
}

/// Feed every token of `text` to `b`.
pub(super) fn lex<'a>(text: &'a str, b: &mut Builder<'a>) -> Result<(), LexError> {
    let bytes = text.as_bytes();
    let mut open: Vec<&'a str> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let mut lt = at;
        if bytes[at] != b'<' {
            let stop = find2(bytes, at, b'<', b'&').unwrap_or(text.len());
            let amp = bytes.get(stop) == Some(&b'&');
            lt = if amp {
                text[stop..].find('<').map_or(text.len(), |i| stop + i)
            } else {
                stop
            };
            if !open.is_empty() {
                let raw = &text[at..lt];
                b.text(if amp {
                    unescape_from(raw, stop - at)?
                } else {
                    Cow::Borrowed(raw)
                })?;
            }
            if lt == text.len() {
                break;
            }
        }
        at = match bytes.get(lt + 1) {
            Some(b'/') => {
                let end = close_at(text, lt + 2)?;
                let name = text[lt + 2..end].trim_end();
                match open.pop() {
                    Some(o) if o == name => {}
                    Some(o) => return Err(format!("expected </{o}>, found </{name}>")),
                    None => return Err(format!("</{name}> closes nothing")),
                }
                b.close()?;
                end + 1
            }
            Some(b'?') => skip_past(text, lt + 2, "?>", "an unterminated processing instruction")?,
            Some(b'!') => special(text, lt, b)?,
            _ => start(text, lt + 1, b, &mut open)?,
        };
    }
    match open.last() {
        Some(name) => Err(format!("<{name}> is never closed")),
        None => Ok(()),
    }
}

fn close_at(text: &str, from: usize) -> Result<usize, LexError> {
    find(text.as_bytes(), from, b'>').ok_or_else(|| "an unterminated tag".to_owned())
}

fn skip_past(text: &str, from: usize, end: &str, what: &str) -> Result<usize, LexError> {
    text[from..]
        .find(end)
        .map(|i| from + i + end.len())
        .ok_or_else(|| what.to_owned())
}

/// `<!--`, `<![CDATA[` or `<!DOCTYPE` at `lt`.
fn special<'a>(text: &'a str, lt: usize, b: &mut Builder<'a>) -> Result<usize, LexError> {
    let rest = &text[lt..];
    if rest.starts_with("<!--") {
        return skip_past(text, lt + 4, "-->", "an unterminated comment");
    }
    if rest.starts_with("<![CDATA[") {
        let from = lt + 9;
        let end = skip_past(text, from, "]]>", "an unterminated CDATA section")?;
        b.text(Cow::Borrowed(&text[from..end - 3]))?;
        return Ok(end);
    }
    if rest.starts_with("<!DOCTYPE") {
        let close = close_at(text, lt)?;
        return match text[lt..close].find('[') {
            Some(_) => skip_past(text, lt, "]", "an unterminated document type")
                .and_then(|after| close_at(text, after).map(|c| c + 1)),
            None => Ok(close + 1),
        };
    }
    Err("an unknown markup declaration".to_owned())
}

const fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).copied().is_some_and(is_space) {
        at += 1;
    }
    at
}

fn name_end(bytes: &[u8], mut at: usize) -> usize {
    while bytes
        .get(at)
        .is_some_and(|c| !is_space(*c) && !matches!(c, b'>' | b'/' | b'='))
    {
        at += 1;
    }
    at
}

/// A start tag whose name begins at `from`.
fn start<'a>(
    text: &'a str,
    from: usize,
    b: &mut Builder<'a>,
    open: &mut Vec<&'a str>,
) -> Result<usize, LexError> {
    let bytes = text.as_bytes();
    let mut at = name_end(bytes, from);
    let name = &text[from..at];
    if name.is_empty() {
        return Err("a tag without a name".to_owned());
    }
    let mark = b.attr_mark();
    loop {
        at = skip_space(bytes, at);
        match bytes.get(at) {
            Some(b'>') => {
                b.open(name, mark)?;
                open.push(name);
                return Ok(at + 1);
            }
            Some(b'/') if bytes.get(at + 1) == Some(&b'>') => {
                b.open(name, mark)?;
                b.close()?;
                return Ok(at + 2);
            }
            Some(_) => at = attribute(text, at, b, mark)?,
            None => return Err(format!("<{name}> is never finished")),
        }
    }
}

/// One `key="value"` starting at `from`; returns where it ends.
fn attribute<'a>(
    text: &'a str,
    from: usize,
    b: &mut Builder<'a>,
    mark: usize,
) -> Result<usize, LexError> {
    let bytes = text.as_bytes();
    let key_end = name_end(bytes, from);
    let key = &text[from..key_end];
    let eq = skip_space(bytes, key_end);
    if key.is_empty() || bytes.get(eq) != Some(&b'=') {
        return Err(format!("attribute {key:?} without a value"));
    }
    let open = skip_space(bytes, eq + 1);
    let quote = match bytes.get(open) {
        Some(q @ (b'"' | b'\'')) => *q,
        _ => return Err(format!("attribute {key} with an unquoted value")),
    };
    let close =
        find(bytes, open + 1, quote).ok_or_else(|| format!("attribute {key} never ends"))?;
    if b.has_attr_since(mark, key) {
        return Err(format!("duplicate attribute {key}"));
    }
    b.attr(key, unescape(&text[open + 1..close])?);
    Ok(close + 1)
}

/// Resolve entity and character references; borrowed when there are none.
fn unescape(raw: &str) -> Result<Cow<'_, str>, LexError> {
    match find(raw.as_bytes(), 0, b'&') {
        None => Ok(Cow::Borrowed(raw)),
        Some(first) => unescape_from(raw, first),
    }
}

/// Resolve references in `raw`, whose first `&` is at `first`.
fn unescape_from(raw: &str, first: usize) -> Result<Cow<'_, str>, LexError> {
    let mut out = String::with_capacity(raw.len());
    out.push_str(&raw[..first]);
    let mut rest = &raw[first..];
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let semi = rest[amp..]
            .find(';')
            .ok_or_else(|| "an entity reference without ';'".to_owned())?;
        out.push(entity(&rest[amp + 1..amp + semi])?);
        rest = &rest[amp + semi + 1..];
    }
    out.push_str(rest);
    Ok(Cow::Owned(out))
}

fn entity(name: &str) -> Result<char, LexError> {
    let code = |digits: &str, radix: u32| {
        u32::from_str_radix(digits, radix)
            .ok()
            .filter(|_| !digits.starts_with(['+', '-']))
            .and_then(char::from_u32)
            .filter(|c| *c != '\0')
            .ok_or_else(|| format!("invalid character reference &{name};"))
    };
    match name {
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "amp" => Ok('&'),
        "apos" => Ok('\''),
        "quot" => Ok('"'),
        _ => match name.strip_prefix('#') {
            Some(hex) if hex.starts_with('x') => code(&hex[1..], 16),
            Some(dec) => code(dec, 10),
            None => Err(format!("unknown entity &{name};")),
        },
    }
}

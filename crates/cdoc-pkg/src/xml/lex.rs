//! The tokeniser: XML text, already checked to be UTF-8, straight into a [`Builder`]. Names,
//! attribute values and text are slices of the input unless an entity had to be resolved.
//!
//! It accepts what the package formats write: elements, attributes in either quote, the five
//! predefined entities and character references, CDATA, comments, processing instructions and
//! a document type declaration (skipped). It refuses unbalanced or mismatched tags, duplicate
//! attributes, an attribute without a quoted value and unknown entities, as a conforming parser
//! must; it does not check that names are made of name characters.

use super::builder::{Builder, Shape};
use super::entity::{unescape, unescape_from};
use super::scan::{find, find2};
use std::borrow::Cow;

/// Why the text is not XML the tokeniser accepts.
pub(super) type LexError = String;

impl From<Shape> for LexError {
    fn from(shape: Shape) -> Self {
        match shape {
            Shape::Unbalanced => "unbalanced end tag or content after the document element",
            Shape::TooMany => "too many nodes",
            Shape::TooDeep => "elements nested deeper than the limit",
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
            let raw = &text[at..lt];
            if !open.is_empty() {
                b.text(if amp {
                    unescape_from(raw, stop - at)?
                } else {
                    Cow::Borrowed(raw)
                })?;
            } else if !raw.bytes().all(is_space) {
                return Err("text outside the document element".to_owned());
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
            Some(b'?') if name_end(bytes, lt + 2) == lt + 2 => {
                return Err("a processing instruction without a target".to_owned());
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
    if let Some(decl) = rest.strip_prefix("<!DOCTYPE") {
        let name_at = skip_space(text.as_bytes(), lt + 9);
        if name_at == lt + 9 || name_end(text.as_bytes(), name_at) == name_at {
            return Err("a document type without a name".to_owned());
        }
        return match decl.find(['[', '<', '>']).map(|i| decl.as_bytes()[i]) {
            Some(b'>') => close_at(text, lt).map(|c| c + 1),
            Some(b'[') => {
                let after = skip_past(text, lt, "]", "an unterminated document type")?;
                let gt = skip_space(text.as_bytes(), after);
                match text.as_bytes().get(gt) {
                    Some(b'>') => Ok(gt + 1),
                    _ => Err("a document type not closed after its internal subset".to_owned()),
                }
            }
            _ => Err("an unterminated document type".to_owned()),
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
        .is_some_and(|c| !is_space(*c) && !matches!(c, b'>' | b'/' | b'=' | b'"' | b'\'' | b'<'))
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
                open_element(b, name, mark)?;
                open.push(name);
                return Ok(at + 1);
            }
            Some(b'/') if bytes.get(at + 1) == Some(&b'>') => {
                open_element(b, name, mark)?;
                b.close()?;
                return Ok(at + 2);
            }
            Some(_) => at = attribute(text, at, b, mark)?,
            None => return Err(format!("<{name}> is never finished")),
        }
    }
}

fn open_element<'a>(b: &mut Builder<'a>, name: &'a str, mark: usize) -> Result<(), LexError> {
    if let Some(key) = b.duplicate_since(mark) {
        return Err(format!("duplicate attribute {key}"));
    }
    Ok(b.open(name, mark)?)
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
        find2(bytes, open + 1, quote, b'<').ok_or_else(|| format!("attribute {key} never ends"))?;
    if bytes[close] == b'<' {
        return Err(format!("attribute {key} holds a '<'"));
    }
    if b.has_attr_since(mark, key) {
        return Err(format!("duplicate attribute {key}"));
    }
    b.attr(key, unescape(&text[open + 1..close])?);
    Ok(close + 1)
}

//! Entity and character references. Only the five predefined entities and character
//! references resolve: entities a document type declares are refused rather than expanded,
//! so no part can grow on reading (no "billion laughs").

use super::lex::LexError;
use super::scan::find;
use std::borrow::Cow;

/// Resolve entity and character references; borrowed when there are none.
pub(super) fn unescape(raw: &str) -> Result<Cow<'_, str>, LexError> {
    match find(raw.as_bytes(), 0, b'&') {
        None => Ok(Cow::Borrowed(raw)),
        Some(first) => unescape_from(raw, first),
    }
}

/// Resolve references in `raw`, whose first `&` is at `first`.
pub(super) fn unescape_from(raw: &str, first: usize) -> Result<Cow<'_, str>, LexError> {
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

//! `RunProperties` field by field: each field is its own mark key. The destructuring below is
//! exhaustive, so a field added to the model fails to compile here until it has a key.

use crate::codec::{from_bytes, to_bytes, CodecError};
use crate::marks::RunKey;
use cdoc_model::RunProperties;

/// Every field that is set, with its encoded value.
pub(crate) fn decompose(p: &RunProperties) -> Result<Vec<(RunKey, Vec<u8>)>, CodecError> {
    let RunProperties {
        style,
        font,
        size,
        bold,
        italic,
        underline,
        strike,
        vertical_align,
        color,
        highlight,
        small_caps,
        all_caps,
    } = p;
    let mut out = Vec::new();
    let mut put =
        |key: RunKey, bytes: Option<Result<Vec<u8>, CodecError>>| -> Result<(), CodecError> {
            if let Some(bytes) = bytes {
                out.push((key, bytes?));
            }
            Ok(())
        };
    put(RunKey::Style, style.as_ref().map(to_bytes))?;
    put(RunKey::Font, font.as_ref().map(to_bytes))?;
    put(RunKey::Size, size.as_ref().map(to_bytes))?;
    put(RunKey::Bold, bold.as_ref().map(to_bytes))?;
    put(RunKey::Italic, italic.as_ref().map(to_bytes))?;
    put(RunKey::Underline, underline.as_ref().map(to_bytes))?;
    put(RunKey::Strike, strike.as_ref().map(to_bytes))?;
    put(RunKey::VerticalAlign, vertical_align.as_ref().map(to_bytes))?;
    put(RunKey::Color, color.as_ref().map(to_bytes))?;
    put(RunKey::Highlight, highlight.as_ref().map(to_bytes))?;
    put(RunKey::SmallCaps, small_caps.as_ref().map(to_bytes))?;
    put(RunKey::AllCaps, all_caps.as_ref().map(to_bytes))?;
    Ok(out)
}

/// Sets one field from its encoded value.
pub(crate) fn set(p: &mut RunProperties, key: RunKey, bytes: &[u8]) -> Result<(), CodecError> {
    match key {
        RunKey::Style => p.style = Some(from_bytes(bytes)?),
        RunKey::Font => p.font = Some(from_bytes(bytes)?),
        RunKey::Size => p.size = Some(from_bytes(bytes)?),
        RunKey::Bold => p.bold = Some(from_bytes(bytes)?),
        RunKey::Italic => p.italic = Some(from_bytes(bytes)?),
        RunKey::Underline => p.underline = Some(from_bytes(bytes)?),
        RunKey::Strike => p.strike = Some(from_bytes(bytes)?),
        RunKey::VerticalAlign => p.vertical_align = Some(from_bytes(bytes)?),
        RunKey::Color => p.color = Some(from_bytes(bytes)?),
        RunKey::Highlight => p.highlight = Some(from_bytes(bytes)?),
        RunKey::SmallCaps => p.small_caps = Some(from_bytes(bytes)?),
        RunKey::AllCaps => p.all_caps = Some(from_bytes(bytes)?),
    }
    Ok(())
}

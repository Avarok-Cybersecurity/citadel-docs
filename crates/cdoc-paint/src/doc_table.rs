//! The document table as read: title, language and outline, between the string table and the
//! page index.

use crate::codec::Reader;
use crate::op::DocumentInfo;
use crate::DecodeError;
use cdoc_layout::OutlineEntry;

fn string(strings: &[&str], index: usize) -> Result<String, DecodeError> {
    strings
        .get(index)
        .map(|s| (*s).to_owned())
        .ok_or(DecodeError::Malformed("string index out of range"))
}

fn optional(r: &mut Reader<'_>, strings: &[&str]) -> Result<Option<String>, DecodeError> {
    match r.len()? {
        0 => Ok(None),
        n => string(strings, n - 1).map(Some),
    }
}

/// Reads the document table, which must end exactly where the page index begins.
pub fn read_document(mut r: Reader<'_>, strings: &[&str]) -> Result<DocumentInfo, DecodeError> {
    let title = optional(&mut r, strings)?;
    let language = optional(&mut r, strings)?;
    let count = r.len()?;
    let mut outline = Vec::with_capacity(count.min(strings.len()));
    for _ in 0..count {
        let level = r.u8()?;
        let page = r.uv()?;
        let y = r.zz()?;
        let index = r.len()?;
        outline.push(OutlineEntry {
            level,
            title: string(strings, index)?,
            page,
            y,
        });
    }
    if r.is_empty() {
        Ok(DocumentInfo {
            title,
            language,
            outline,
        })
    } else {
        Err(DecodeError::Malformed(
            "document table longer than its entries",
        ))
    }
}

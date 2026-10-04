//! The end of an existing file: where its last cross-reference section is, whether that is a
//! table or a stream, and what its trailer says (size, root, info, id, encryption).

use super::UpdateError;
use hayro_syntax::object::{Array, Dict, FromBytes, MaybeRef, ObjRef};

/// What an update must know about the file it is appended to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tail {
    /// Offset of the last cross-reference section: the new trailer's `/Prev`.
    pub startxref: usize,
    /// The trailer's `/Size`: the first free object number.
    pub size: i32,
    /// The catalog.
    pub root: ObjRef,
    /// The information dictionary, if any.
    pub info: Option<ObjRef>,
    /// The `/ID` array, verbatim with its brackets, if any.
    pub id: Option<Vec<u8>>,
    /// Whether the last section is a cross-reference stream.
    pub stream: bool,
    /// Whether the trailer names an `/Encrypt` dictionary.
    pub encrypted: bool,
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn startxref(bytes: &[u8]) -> Option<usize> {
    let from = bytes.len().saturating_sub(2048);
    let at = from + find_last(&bytes[from..], b"startxref")? + b"startxref".len();
    let digits: String = bytes[at..]
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .take_while(|b| b.is_ascii_digit())
        .map(|b| char::from(*b))
        .collect();
    digits.parse().ok().filter(|o| *o < bytes.len())
}

/// Reads the file's tail.
pub fn read(bytes: &[u8]) -> Result<Tail, UpdateError> {
    let offset = startxref(bytes).ok_or(UpdateError::NoStartXref)?;
    let section = &bytes[offset..];
    let stream = !section.starts_with(b"xref");
    let dict_from = if stream {
        find(section, b"<<").ok_or(UpdateError::BadTrailer)?
    } else {
        let t = find(section, b"trailer").ok_or(UpdateError::BadTrailer)?;
        t + find(&section[t..], b"<<").ok_or(UpdateError::BadTrailer)?
    };
    let dict = Dict::from_bytes(&section[dict_from..]).ok_or(UpdateError::BadTrailer)?;
    let size = dict.get::<i32>("Size").ok_or(UpdateError::BadTrailer)?;
    let root = dict.get_ref("Root").ok_or(UpdateError::BadTrailer)?;
    let id = match dict.get_raw::<Array<'_>>("ID") {
        // hayro hands over an array's contents without its brackets.
        Some(MaybeRef::NotRef(array)) => Some([b"[", array.data(), b"]"].concat()),
        _ => None,
    };
    Ok(Tail {
        startxref: offset,
        size,
        root,
        info: dict.get_ref("Info"),
        id,
        stream,
        encrypted: dict.contains_key("Encrypt"),
    })
}

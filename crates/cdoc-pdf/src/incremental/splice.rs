//! Adding object references to an array-valued entry of an existing dictionary without
//! re-serialising anything else: the dictionary's own bytes are copied, and only the array is
//! rewritten (or inserted). What is not understood is copied verbatim, so an update never
//! loses an entry it did not know about.

use super::UpdateError;
use hayro_syntax::object::{Array, Dict, MaybeRef, ObjectIdentifier};
use hayro_syntax::xref::XRef;
use pdf_writer::Ref;

/// The result of adding references to `dict[key]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spliced {
    /// The dictionary's new bytes (the array was absent or direct).
    Inline(Vec<u8>),
    /// The array is an object of its own: its new bytes, under its own identifier.
    Referenced(ObjectIdentifier, Vec<u8>),
}

/// `" 12 0 R 13 0 R"`.
pub fn ref_list(refs: &[Ref]) -> String {
    use std::fmt::Write as _;
    refs.iter().fold(String::new(), |mut out, r| {
        let _ = write!(out, " {} 0 R", r.get());
        out
    })
}

/// Where `inner` sits inside `outer`, when it is a slice of it.
fn offset_in(outer: &[u8], inner: &[u8]) -> Option<usize> {
    let start = (inner.as_ptr() as usize).checked_sub(outer.as_ptr() as usize)?;
    (start + inner.len() <= outer.len()).then_some(start)
}

/// `outer` with the slice `inner` replaced by `with`.
pub fn replace(outer: &[u8], inner: &[u8], with: &[u8]) -> Result<Vec<u8>, UpdateError> {
    let start = offset_in(outer, inner).ok_or(UpdateError::Unsupported("a detached object"))?;
    Ok([&outer[..start], with, &outer[start + inner.len()..]].concat())
}

/// An array's contents (hayro hands them over without the brackets) with `refs` appended.
fn extended(contents: &[u8], refs: &[Ref]) -> Vec<u8> {
    let kept = contents.trim_ascii_end();
    let added = ref_list(refs);
    let added = if kept.is_empty() {
        added.trim_start()
    } else {
        &added
    };
    [kept, added.as_bytes()].concat()
}

/// `dict` with `/key` inserted as the first entry.
pub fn insert(dict: &[u8], key: &str, value: &str) -> Result<Vec<u8>, UpdateError> {
    let rest = dict.strip_prefix(b"<<").ok_or(UpdateError::Unsupported(
        "a dictionary without its brackets",
    ))?;
    Ok([
        b"<</".as_slice(),
        key.as_bytes(),
        b" ",
        value.as_bytes(),
        b" ",
        rest,
    ]
    .concat())
}

/// Adds `refs` to the array at `dict[key]`, creating it when absent.
pub fn append_refs(
    dict: &Dict<'_>,
    xref: &XRef,
    key: &str,
    refs: &[Ref],
) -> Result<Spliced, UpdateError> {
    let raw = dict.data();
    match dict.get_raw::<Array<'_>>(key) {
        None => Ok(Spliced::Inline(insert(
            raw,
            key,
            &format!("[{}]", ref_list(refs).trim_start()),
        )?)),
        Some(MaybeRef::NotRef(array)) => {
            let grown = extended(array.data(), refs);
            Ok(Spliced::Inline(replace(raw, array.data(), &grown)?))
        }
        Some(MaybeRef::Ref(r)) => {
            let id: ObjectIdentifier = r.into();
            let array = xref
                .get::<Array<'_>>(id)
                .ok_or(UpdateError::Unsupported("an unreadable referenced array"))?;
            let body = [b"[".as_slice(), &extended(array.data(), refs), b"]"].concat();
            Ok(Spliced::Referenced(id, body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayro_syntax::object::FromBytes;

    #[test]
    fn an_absent_key_is_inserted_and_everything_else_kept_verbatim() {
        let out = insert(b"<</Type /Page /Odd (x)>>", "Annots", "[7 0 R]").expect("inserts");
        assert_eq!(out, b"<</Annots [7 0 R] /Type /Page /Odd (x)>>");
    }

    #[test]
    fn a_direct_array_grows_in_place() {
        let raw = b"<</Annots [1 0 R] /Type /Page>>";
        let dict = Dict::from_bytes(raw).expect("dict");
        let Some(MaybeRef::NotRef(array)) = dict.get_raw::<Array<'_>>("Annots") else {
            panic!("direct");
        };
        let grown = extended(array.data(), &[Ref::new(9)]);
        let out = replace(dict.data(), array.data(), &grown).expect("splices");
        assert_eq!(out, b"<</Annots [1 0 R 9 0 R] /Type /Page>>");
    }
}

//! The result's formatting section (version 2): effective and direct character and paragraph
//! properties at the selection start, and the properties that differ across the selection.
//!
//! ```text
//! formatting := run:value run_direct:value paragraph:value paragraph_direct:value
//!               run_mixed:varint paragraph_mixed:varint
//! ```
//!
//! Bit `i` of `run_mixed` is `RunKey::ALL[i]`, bit `i` of `paragraph_mixed` is
//! `ParagraphKey::ALL[i]`; a bit past the last key is malformed.

use super::{Reader, Writer};
use crate::caret::{CaretFormatting, ParagraphKey};
use cdoc_crdt::{DecodeError, RunKey};

fn bits<K: PartialEq>(all: &[K], set: &[K]) -> u64 {
    all.iter()
        .enumerate()
        .filter(|(_, k)| set.contains(k))
        .fold(0, |acc, (i, _)| acc | 1 << i)
}

fn keys<K: Copy>(all: &[K], bits: u64) -> Result<Vec<K>, DecodeError> {
    if bits >> all.len() != 0 {
        return Err(DecodeError::OutOfRange("mixed set"));
    }
    Ok(all
        .iter()
        .enumerate()
        .filter(|(i, _)| bits >> i & 1 == 1)
        .map(|(_, k)| *k)
        .collect())
}

pub(super) fn write(w: &mut Writer, f: &CaretFormatting) {
    value!(w, &f.run);
    value!(w, &f.run_direct);
    value!(w, &f.paragraph);
    value!(w, &f.paragraph_direct);
    w.uvar(bits(&RunKey::ALL, &f.mixed_run));
    w.uvar(bits(&ParagraphKey::ALL, &f.mixed_paragraph));
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<CaretFormatting, DecodeError> {
    Ok(CaretFormatting {
        run: read_value!(r),
        run_direct: read_value!(r),
        paragraph: read_value!(r),
        paragraph_direct: read_value!(r),
        mixed_run: keys(&RunKey::ALL, r.uvar()?)?,
        mixed_paragraph: keys(&ParagraphKey::ALL, r.uvar()?)?,
    })
}

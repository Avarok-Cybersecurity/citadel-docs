//! The result's formatting section (version 5): effective and direct character and paragraph
//! properties at the selection start, the list there, and what differs across the selection.
//!
//! ```text
//! formatting := run:value run_direct:value paragraph:value paragraph_direct:value
//!               run_mixed:varint paragraph_mixed:varint
//!               list:(0 | 1 kind:u8 level:u8) list_mixed:u8
//! ```
//!
//! Bit `i` of `run_mixed` is `RunKey::ALL[i]`, bit `i` of `paragraph_mixed` is
//! `ParagraphKey::ALL[i]`; a bit past the last key is malformed. A list kind below 6 is
//! `ListKind::ALL[kind]` (the codes of `SetNumberingKind`), 6 is a custom definition.

use super::{Reader, Writer};
use crate::caret::{CaretFormatting, ParagraphKey};
use crate::caret_list::{CaretList, CaretListKind};
use cdoc_crdt::{DecodeError, RunKey};
use cdoc_model::builtin::ListKind;

const CUSTOM: u8 = 6;

fn write_list(w: &mut Writer, list: Option<CaretList>) {
    let Some(list) = list else {
        w.u8(0);
        return;
    };
    w.u8(1);
    w.u8(match list.kind {
        CaretListKind::Builtin(kind) => super::tables::code(&ListKind::ALL, &kind),
        CaretListKind::Custom => CUSTOM,
    });
    w.u8(list.level);
}

fn read_list(r: &mut Reader<'_>) -> Result<Option<CaretList>, DecodeError> {
    match r.u8()? {
        0 => Ok(None),
        1 => {
            let code = r.u8()?;
            let kind = match ListKind::ALL.get(usize::from(code)) {
                Some(kind) => CaretListKind::Builtin(*kind),
                None if code == CUSTOM => CaretListKind::Custom,
                None => {
                    return Err(DecodeError::BadTag {
                        what: "list kind",
                        tag: code,
                    })
                }
            };
            let level = r.u8()?;
            if level > cdoc_model::MAX_LEVEL {
                return Err(DecodeError::OutOfRange("list level"));
            }
            Ok(Some(CaretList { kind, level }))
        }
        tag => Err(DecodeError::BadTag { what: "list", tag }),
    }
}

fn flag(r: &mut Reader<'_>, what: &'static str) -> Result<bool, DecodeError> {
    match r.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(DecodeError::BadTag { what, tag }),
    }
}

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
    write_list(w, f.list);
    w.u8(u8::from(f.mixed_list));
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<CaretFormatting, DecodeError> {
    Ok(CaretFormatting {
        run: read_value!(r),
        run_direct: read_value!(r),
        paragraph: read_value!(r),
        paragraph_direct: read_value!(r),
        mixed_run: keys(&RunKey::ALL, r.uvar()?)?,
        mixed_paragraph: keys(&ParagraphKey::ALL, r.uvar()?)?,
        list: read_list(r)?,
        mixed_list: flag(r, "list mixed")?,
    })
}

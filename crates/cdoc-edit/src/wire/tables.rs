//! The value tables of the request and result records: a value's code is its index.

use super::Reader;
use crate::commands::{DeleteUnit, MarkKind};
use crate::position::Granularity;
use cdoc_crdt::{DecodeError, Furniture};
use cdoc_model::builtin::ListKind;
use cdoc_model::{BreakKind, HeaderFooterKind};

pub(super) const UNITS: [DeleteUnit; 2] = [DeleteUnit::Char, DeleteUnit::Word];
pub(super) const BREAKS: [BreakKind; 3] = [BreakKind::Line, BreakKind::Page, BreakKind::Column];
pub(super) const MARKS: [MarkKind; 6] = [
    MarkKind::Bold,
    MarkKind::Italic,
    MarkKind::Underline,
    MarkKind::Strike,
    MarkKind::Subscript,
    MarkKind::Superscript,
];
pub(super) const LISTS: [ListKind; 6] = ListKind::ALL;
pub(super) const FURNITURE: [Furniture; 2] = [Furniture::Header, Furniture::Footer];
pub(super) const HEADER_KINDS: [HeaderFooterKind; 3] = [
    HeaderFooterKind::Default,
    HeaderFooterKind::First,
    HeaderFooterKind::Even,
];
pub(super) const GRANULARITIES: [Granularity; 6] = [
    Granularity::Char,
    Granularity::Word,
    Granularity::LineBoundary,
    Granularity::Line,
    Granularity::Paragraph,
    Granularity::Document,
];

pub(super) fn code<T: PartialEq>(table: &[T], v: &T) -> u8 {
    u8::try_from(
        table
            .iter()
            .position(|t| t == v)
            .expect("every value is in its table"),
    )
    .expect("small tables")
}

pub(super) fn pick<T: Copy>(
    table: &[T],
    r: &mut Reader<'_>,
    what: &'static str,
) -> Result<T, DecodeError> {
    let tag = r.u8()?;
    table
        .get(usize::from(tag))
        .copied()
        .ok_or(DecodeError::BadTag { what, tag })
}

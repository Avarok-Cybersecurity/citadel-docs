//! The reader: borrows the bytes, checks the header and tables once, and checks each page body
//! the first time it is asked for, so iterating a page's ops cannot fail and pages never
//! asked for are never touched.

use crate::codec::Reader;
use crate::face_table::read_faces;
pub use crate::face_table::FaceView;
use crate::format::{HEADER_LEN, INDEX_ENTRY_LEN, MAGIC, VERSION};
use crate::op::{DisplayList, PageOps};
use crate::ops_view::OpIter;
use crate::parse::validate_body;
use crate::DecodeError;
use cdoc_layout::Pt100;

/// A decoded display list over borrowed bytes.
#[derive(Debug, Clone)]
pub struct DisplayListView<'a> {
    bytes: &'a [u8],
    faces: Vec<FaceView<'a>>,
    strings: Vec<&'a str>,
    index: &'a [u8],
    index_end: usize,
}

/// One page, its body already checked.
#[derive(Debug, Clone, Copy)]
pub struct PageView<'a> {
    /// The zero-based page number in the layout.
    pub number: u32,
    /// Width.
    pub width: Pt100,
    /// Height.
    pub height: Pt100,
    body: &'a [u8],
    strings: &'a [&'a str],
    faces: &'a [FaceView<'a>],
}

fn utf8(bytes: &[u8]) -> Result<&str, DecodeError> {
    std::str::from_utf8(bytes).map_err(|_| DecodeError::Malformed("string is not UTF-8"))
}

fn offset(value: u32) -> Result<usize, DecodeError> {
    usize::try_from(value).map_err(|_| DecodeError::Malformed("offset"))
}

/// Reads the header, face table, string table and page index. Page bodies are checked when
/// [`DisplayListView::page`] asks for them.
pub fn decode(bytes: &[u8]) -> Result<DisplayListView<'_>, DecodeError> {
    let mut header = Reader::new(bytes, 0);
    if header.take(4).map_err(|_| DecodeError::BadMagic)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = header.u16()?;
    if version != VERSION {
        return Err(DecodeError::Version(version));
    }
    if header.u16()? != 0 {
        return Err(DecodeError::Malformed("reserved header flags are set"));
    }
    let pages = offset(header.u32()?)?;
    let [faces_at, strings_at, index_at] = [header.u32()?, header.u32()?, header.u32()?];
    let [faces_at, strings_at, index_at] =
        [offset(faces_at)?, offset(strings_at)?, offset(index_at)?];
    if faces_at != HEADER_LEN || strings_at < faces_at || index_at < strings_at {
        return Err(DecodeError::Malformed("section offsets out of order"));
    }
    let section = |from: usize, to: usize| {
        bytes
            .get(from..to)
            .map(|s| Reader::new(s, from))
            .ok_or(DecodeError::Truncated(bytes.len()))
    };
    let faces = read_faces(section(faces_at, strings_at)?, strings_at - faces_at)?;
    let mut r = section(strings_at, index_at)?;
    let count = r.len()?;
    let mut strings = Vec::with_capacity(count.min(index_at - strings_at));
    for _ in 0..count {
        let len = r.len()?;
        strings.push(utf8(r.take(len)?)?);
    }
    if !r.is_empty() {
        return Err(DecodeError::Malformed(
            "string table longer than its entries",
        ));
    }
    let index_len = pages
        .checked_mul(INDEX_ENTRY_LEN)
        .ok_or(DecodeError::Malformed("page count"))?;
    let index = bytes
        .get(index_at..index_at.saturating_add(index_len))
        .ok_or(DecodeError::Truncated(bytes.len()))?;
    Ok(DisplayListView {
        bytes,
        faces,
        strings,
        index,
        index_end: index_at + index_len,
    })
}

impl<'a> DisplayListView<'a> {
    /// The face table, by face id.
    #[must_use]
    pub fn faces(&self) -> &[FaceView<'a>] {
        &self.faces
    }

    /// How many pages the list holds.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.index.len() / INDEX_ENTRY_LEN
    }

    /// The page in `slot` (its position in the list, not its page number), body checked.
    pub fn page(&self, slot: usize) -> Result<PageView<'_>, DecodeError> {
        let entry = self
            .index
            .get(slot * INDEX_ENTRY_LEN..(slot + 1) * INDEX_ENTRY_LEN)
            .ok_or(DecodeError::NoPage(slot))?;
        let mut r = Reader::new(entry, 0);
        let (number, at, len) = (r.u32()?, offset(r.u32()?)?, offset(r.u32()?)?);
        let (width, height) = (r.i32()?, r.i32()?);
        if at < self.index_end {
            return Err(DecodeError::Malformed("page body overlaps the tables"));
        }
        let body = at
            .checked_add(len)
            .and_then(|end| self.bytes.get(at..end))
            .ok_or(DecodeError::Truncated(self.bytes.len()))?;
        validate_body(body, at, &self.faces, &self.strings)?;
        Ok(PageView {
            number,
            width,
            height,
            body,
            strings: &self.strings,
            faces: &self.faces,
        })
    }

    /// Every page, each body checked, as an owned [`DisplayList`].
    pub fn to_owned(&self) -> Result<DisplayList, DecodeError> {
        let pages = (0..self.page_count())
            .map(|slot| {
                let page = self.page(slot)?;
                Ok(PageOps {
                    number: page.number,
                    width: page.width,
                    height: page.height,
                    ops: page.ops().map(|op| op.to_owned()).collect(),
                })
            })
            .collect::<Result<_, DecodeError>>()?;
        Ok(DisplayList {
            faces: self.faces.iter().map(FaceView::to_owned).collect(),
            pages,
        })
    }
}

impl<'a> PageView<'a> {
    /// The page's operations, in painting order.
    #[must_use]
    pub fn ops(&self) -> OpIter<'a> {
        OpIter::new(self.body, self.strings, self.faces)
    }

    /// The encoded body's size in bytes.
    #[must_use]
    pub const fn body_len(&self) -> usize {
        self.body.len()
    }
}

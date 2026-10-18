//! Operations read in place: [`OpView`] borrows its strings, glyphs and path segments from the
//! page body. A body is checked once (`parse::validate_body`) before any iterator is handed
//! out, so the iterators here yield values, not results.

use crate::caret::CaretLinesView;
use crate::codec::{Decode, Reader};
use crate::face_table::FaceView;
use crate::op::{Op, Rgba, RunHead};
use crate::parse::next_op;
use crate::run_view::ShapedGlyphs;
use crate::strike::StrikeView;
use crate::DecodeError;
use cdoc_layout::{Pt100, Rect, Segment};

/// A counted run of items read lazily from a payload.
#[derive(Debug, Clone)]
pub struct Items<'a, T> {
    pub(crate) r: Reader<'a>,
    pub(crate) left: usize,
    pub(crate) item: std::marker::PhantomData<T>,
}

/// A path's segments, read in place.
pub type Segments<'a> = Items<'a, Segment>;

impl<'a, T: Decode> Items<'a, T> {
    /// `left` items starting at `r`.
    pub(crate) const fn counted(r: Reader<'a>, left: usize) -> Self {
        Self {
            r,
            left,
            item: std::marker::PhantomData,
        }
    }

    #[inline]
    pub(crate) fn checked(&mut self) -> Result<Option<T>, DecodeError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        T::decode(&mut self.r).map(Some)
    }

    /// Reads every item and requires the payload to end exactly after the last.
    pub(crate) fn check(self) -> Result<(), DecodeError> {
        self.check_each(|_| Ok(()))
    }

    /// [`Self::check`], handing each item to `each` in the same pass.
    pub(crate) fn check_each(
        mut self,
        mut each: impl FnMut(&T) -> Result<(), DecodeError>,
    ) -> Result<(), DecodeError> {
        while let Some(item) = self.checked()? {
            each(&item)?;
        }
        if self.r.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::Malformed("payload longer than its items"))
        }
    }
}

impl<T: Decode> Iterator for Items<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.checked().ok().flatten()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<T: Decode> ExactSizeIterator for Items<'_, T> {}

/// One operation, borrowed from the page body. See [`Op`] for what each paints.
#[derive(Debug, Clone)]
#[expect(
    missing_docs,
    reason = "the fields are those of `Op`, documented there"
)]
pub enum OpView<'a> {
    FillRect {
        rect: Rect,
        colour: Rgba,
    },
    StrokeRect {
        rect: Rect,
        width: Pt100,
        colour: Rgba,
    },
    Line {
        from: (Pt100, Pt100),
        to: (Pt100, Pt100),
        width: Pt100,
        colour: Rgba,
    },
    FillPath {
        segments: Segments<'a>,
        colour: Rgba,
    },
    StrokePath {
        segments: Segments<'a>,
        width: Pt100,
        colour: Rgba,
    },
    GlyphRun {
        head: RunHead,
        text: &'a str,
        glyphs: ShapedGlyphs<'a>,
    },
    Image {
        rect: Rect,
        content_id: &'a str,
    },
    Link {
        rect: Rect,
        href: &'a str,
    },
    SigField {
        rect: Rect,
        id: &'a str,
        role: &'a str,
    },
    ClipPush(Rect),
    ClipPop,
    CaretLines(CaretLinesView<'a>),
}

/// A page's operations; the body was checked when the page was opened.
#[derive(Debug, Clone)]
pub struct OpIter<'a> {
    body: Reader<'a>,
    strings: &'a [&'a str],
    faces: &'a [FaceView<'a>],
    strikes: &'a [StrikeView],
}

impl<'a> OpIter<'a> {
    pub(crate) const fn new(
        body: &'a [u8],
        strings: &'a [&'a str],
        faces: &'a [FaceView<'a>],
        strikes: &'a [StrikeView],
    ) -> Self {
        Self {
            body: Reader::new(body, 0),
            strings,
            faces,
            strikes,
        }
    }
}

impl<'a> Iterator for OpIter<'a> {
    type Item = OpView<'a>;
    fn next(&mut self) -> Option<OpView<'a>> {
        if self.body.is_empty() {
            return None;
        }
        next_op(&mut self.body, self.strings, self.faces, self.strikes).ok()
    }
}

impl OpView<'_> {
    /// The operation as an owned value.
    #[must_use]
    pub fn to_owned(&self) -> Op {
        match self.clone() {
            Self::FillRect { rect, colour } => Op::FillRect { rect, colour },
            Self::StrokeRect {
                rect,
                width,
                colour,
            } => Op::StrokeRect {
                rect,
                width,
                colour,
            },
            Self::Line {
                from,
                to,
                width,
                colour,
            } => Op::Line {
                from,
                to,
                width,
                colour,
            },
            Self::FillPath { segments, colour } => Op::FillPath {
                segments: segments.collect(),
                colour,
            },
            Self::StrokePath {
                segments,
                width,
                colour,
            } => Op::StrokePath {
                segments: segments.collect(),
                width,
                colour,
            },
            Self::GlyphRun { head, text, glyphs } => Op::GlyphRun {
                head,
                text: text.to_owned(),
                glyphs: glyphs.collect(),
            },
            Self::Image { rect, content_id } => Op::Image {
                rect,
                content_id: content_id.to_owned(),
            },
            Self::Link { rect, href } => Op::Link {
                rect,
                href: href.to_owned(),
            },
            Self::SigField { rect, id, role } => Op::SigField {
                rect,
                id: id.to_owned(),
                role: role.to_owned(),
            },
            Self::ClipPush(rect) => Op::ClipPush(rect),
            Self::ClipPop => Op::ClipPop,
            Self::CaretLines(lines) => {
                Op::CaretLines(lines.lines().map(|l| l.to_owned()).collect())
            }
        }
    }
}

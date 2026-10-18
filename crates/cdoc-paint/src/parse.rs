//! Reading one operation from a page body, and checking a whole body before any iterator
//! over it is handed out.

use crate::codec::{Decode, Reader};
use crate::face_table::FaceView;
use crate::format::{
    OP_CARET_LINES, OP_CLIP_POP, OP_CLIP_PUSH, OP_FILL_PATH, OP_FILL_RECT, OP_GLYPH_RUN, OP_IMAGE,
    OP_LINE, OP_LINK, OP_SIG_FIELD, OP_STROKE_PATH, OP_STROKE_RECT, SEG_CLOSE, SEG_CUBIC, SEG_LINE,
    SEG_MOVE,
};
use crate::op::Rgba;
use crate::ops_view::{Items, OpView};
use crate::run_view::{parse_run, RunParts};
use crate::strike::StrikeView;
use crate::DecodeError;
use cdoc_layout::{Rect, Segment};

fn rect(r: &mut Reader<'_>) -> Result<Rect, DecodeError> {
    Ok(Rect {
        x: r.zz()?,
        y: r.zz()?,
        width: r.zz()?,
        height: r.zz()?,
    })
}

fn rgba(r: &mut Reader<'_>) -> Result<Rgba, DecodeError> {
    Ok(Rgba {
        r: r.u8()?,
        g: r.u8()?,
        b: r.u8()?,
        a: r.u8()?,
    })
}

impl Decode for Segment {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match r.u8()? {
            SEG_MOVE => Segment::Move(r.zz()?, r.zz()?),
            SEG_LINE => Segment::Line(r.zz()?, r.zz()?),
            SEG_CUBIC => Segment::Cubic([r.zz()?, r.zz()?, r.zz()?, r.zz()?, r.zz()?, r.zz()?]),
            SEG_CLOSE => Segment::Close,
            _ => return Err(DecodeError::Malformed("unknown path verb")),
        })
    }
}

fn items<T>(mut r: Reader<'_>) -> Result<Items<'_, T>, DecodeError> {
    let left = r.len()?;
    Ok(Items {
        r,
        left,
        item: std::marker::PhantomData,
    })
}

/// Parses one op's payload; `Ok(op, rest)` leaves whatever the fixed fields did not use.
fn parse<'a>(
    code: u8,
    mut r: Reader<'a>,
    strings: &[&'a str],
    faces: &'a [FaceView<'a>],
    strikes: &'a [StrikeView],
) -> Result<(OpView<'a>, Reader<'a>), DecodeError> {
    let string = |r: &mut Reader<'_>| -> Result<&'a str, DecodeError> {
        strings
            .get(r.len()?)
            .copied()
            .ok_or(DecodeError::Malformed("string index out of range"))
    };
    let op = match code {
        OP_FILL_RECT => OpView::FillRect {
            rect: rect(&mut r)?,
            colour: rgba(&mut r)?,
        },
        OP_STROKE_RECT => OpView::StrokeRect {
            rect: rect(&mut r)?,
            width: r.zz()?,
            colour: rgba(&mut r)?,
        },
        OP_LINE => OpView::Line {
            from: (r.zz()?, r.zz()?),
            to: (r.zz()?, r.zz()?),
            width: r.zz()?,
            colour: rgba(&mut r)?,
        },
        OP_FILL_PATH => {
            let colour = rgba(&mut r)?;
            let segments = items(r.clone())?;
            return Ok((OpView::FillPath { segments, colour }, Reader::new(&[], 0)));
        }
        OP_STROKE_PATH => {
            let (width, colour) = (r.zz()?, rgba(&mut r)?);
            let segments = items(r.clone())?;
            let op = OpView::StrokePath {
                segments,
                width,
                colour,
            };
            return Ok((op, Reader::new(&[], 0)));
        }
        OP_GLYPH_RUN => {
            let RunParts { head, text, glyphs } = parse_run(r, faces, strikes)?;
            let op = OpView::GlyphRun { head, text, glyphs };
            return Ok((op, Reader::new(&[], 0)));
        }
        OP_IMAGE => OpView::Image {
            rect: rect(&mut r)?,
            content_id: string(&mut r)?,
        },
        OP_LINK => OpView::Link {
            rect: rect(&mut r)?,
            href: string(&mut r)?,
        },
        OP_SIG_FIELD => OpView::SigField {
            rect: rect(&mut r)?,
            id: string(&mut r)?,
            role: string(&mut r)?,
        },
        OP_CLIP_PUSH => OpView::ClipPush(rect(&mut r)?),
        OP_CLIP_POP => OpView::ClipPop,
        OP_CARET_LINES => {
            let lines = crate::caret::parse(&mut r)?;
            return Ok((OpView::CaretLines(lines), Reader::new(&[], 0)));
        }
        other => return Err(DecodeError::UnknownOp(other)),
    };
    Ok((op, r))
}

pub fn next_op<'a>(
    body: &mut Reader<'a>,
    strings: &[&'a str],
    faces: &'a [FaceView<'a>],
    strikes: &'a [StrikeView],
) -> Result<OpView<'a>, DecodeError> {
    let code = body.u8()?;
    let len = body.len()?;
    let payload = Reader::new(body.take(len)?, 0);
    let (op, rest) = parse(code, payload, strings, faces, strikes)?;
    if rest.is_empty() {
        Ok(op)
    } else {
        Err(DecodeError::Malformed("payload longer than its fields"))
    }
}

/// Checks a whole page body: known opcodes, payloads exactly consumed, strike, glyph and
/// string indices in range, every glyph's prediction or exception sound, clip pushes and pops
/// balanced.
pub fn validate_body(
    body: &[u8],
    at: usize,
    faces: &[FaceView<'_>],
    strikes: &[StrikeView],
    strings: &[&str],
) -> Result<(), DecodeError> {
    let mut r = Reader::new(body, at);
    let mut depth: usize = 0;
    while !r.is_empty() {
        match next_op(&mut r, strings, faces, strikes)? {
            OpView::FillPath { segments, .. } | OpView::StrokePath { segments, .. } => {
                segments.check()?;
            }
            OpView::GlyphRun { glyphs, .. } => glyphs.check()?,
            OpView::CaretLines(lines) => crate::caret::check(&lines)?,
            OpView::ClipPush(_) => depth += 1,
            OpView::ClipPop => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(DecodeError::Malformed("clip pop without a push"))?;
            }
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err(DecodeError::Malformed("clip push without a pop"))
    }
}

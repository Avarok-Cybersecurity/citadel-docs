//! `CARET_LINES`: a page's lines of document text as the layout reports them, with where each
//! came from, its caret extent, its runs and the stops no run draws (`docs/display-list.md`).
//! One op per page, so a painter skips them all at once; an editor reads them.

mod view;

pub use view::{check, parse, CaretLineView, CaretLinesView, Lines};

use crate::codec::{Decode, Reader, Writer};
use crate::encode_tables::u32_of;
use crate::format::{
    KIND_DEFAULT, KIND_EVEN, KIND_FIRST, LINE_NEW, LINE_NO_STOPS, LINE_SAME_HEIGHT,
    LINE_SAME_PITCH, REGION_BODY, REGION_FOOTER, REGION_FOOTNOTE, REGION_HEADER,
};
use crate::ops_view::Items;
use crate::{DecodeError, PaintError};
use cdoc_layout::{CaretLine, CaretStop, Provenance, Pt100, Region};
use cdoc_model::{FootnoteId, HeaderFooterKind};

const fn kind_code(kind: HeaderFooterKind) -> u8 {
    match kind {
        HeaderFooterKind::Default => KIND_DEFAULT,
        HeaderFooterKind::First => KIND_FIRST,
        HeaderFooterKind::Even => KIND_EVEN,
    }
}

fn encode_provenance(w: &mut Writer, from: &Provenance) -> Result<(), PaintError> {
    match from.region {
        Region::Body { section } => {
            w.u8(REGION_BODY);
            w.uv(section);
        }
        Region::Header { section, kind } | Region::Footer { section, kind } => {
            let header = matches!(from.region, Region::Header { .. });
            w.u8(if header { REGION_HEADER } else { REGION_FOOTER });
            w.uv(section);
            w.u8(kind_code(kind));
        }
        Region::Footnote(id) => {
            w.u8(REGION_FOOTNOTE);
            w.uv(id.0);
        }
    }
    w.uv(u32_of(from.path.len())?);
    for &block in &from.path {
        w.uv(block);
    }
    Ok(())
}

/// The payload: the line count, then per line its head (the flags of what is the previous
/// line's), its provenance, its top's step from the previous line's, its height, its runs and
/// its stops, each left out where the head says it is the previous line's (for the first line:
/// a step and height of 0) or that there are none.
pub fn encode(w: &mut Writer, lines: &[CaretLine]) -> Result<(), PaintError> {
    w.uv(u32_of(lines.len())?);
    let (mut last, mut pitch, mut height): (Option<&CaretLine>, Pt100, Pt100) = (None, 0, 0);
    for line in lines {
        let step = line.top.wrapping_sub(last.map_or(0, |l| l.top));
        let tall = line.bottom.wrapping_sub(line.top);
        let new = !last.is_some_and(|l| l.from == line.from);
        let mut head = 0;
        for (flag, set) in [
            (LINE_NEW, new),
            (LINE_SAME_PITCH, step == pitch),
            (LINE_SAME_HEIGHT, tall == height),
            (LINE_NO_STOPS, line.stops.is_empty()),
        ] {
            if set {
                head |= flag;
            }
        }
        w.u8(head);
        if new {
            encode_provenance(w, &line.from)?;
        }
        if step != pitch {
            w.zz(step);
        }
        if tall != height {
            w.zz(tall);
        }
        w.uv(line.runs);
        if !line.stops.is_empty() {
            w.uv(u32_of(line.stops.len())?);
            for stop in &line.stops {
                w.uv(stop.gap);
                w.zz(stop.x);
            }
        }
        (last, pitch, height) = (Some(line), step, tall);
    }
    Ok(())
}

impl Decode for u32 {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.uv()
    }
}

impl Decode for CaretStop {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            gap: r.uv()?,
            x: r.zz()?,
        })
    }
}

fn kind(r: &mut Reader<'_>) -> Result<HeaderFooterKind, DecodeError> {
    Ok(match r.u8()? {
        KIND_DEFAULT => HeaderFooterKind::Default,
        KIND_FIRST => HeaderFooterKind::First,
        KIND_EVEN => HeaderFooterKind::Even,
        _ => return Err(DecodeError::Malformed("unknown header or footer kind")),
    })
}

/// A new paragraph's region and path, the path left in place.
pub(super) fn provenance<'a>(r: &mut Reader<'a>) -> Result<(Region, Items<'a, u32>), DecodeError> {
    let region = match r.u8()? {
        REGION_BODY => Region::Body { section: r.uv()? },
        REGION_HEADER => Region::Header {
            section: r.uv()?,
            kind: kind(r)?,
        },
        REGION_FOOTER => Region::Footer {
            section: r.uv()?,
            kind: kind(r)?,
        },
        REGION_FOOTNOTE => Region::Footnote(FootnoteId(r.uv()?)),
        _ => return Err(DecodeError::Malformed("unknown caret line region")),
    };
    let left = r.len()?;
    let path = Items::counted(r.clone(), left);
    for _ in 0..left {
        r.uv()?;
    }
    Ok((region, path))
}

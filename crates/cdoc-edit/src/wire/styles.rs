//! The document's style sheet over the boundary, for the styles gallery.
//!
//! ```text
//! styles   := "CDS" 0x01 count:varint style{count}
//! style    := id:str name:str kind:u8 based_on:(0 | 1 id:str) run:value paragraph:value
//! kind     := 0 paragraph | 1 character | 2 table
//! ```
//!
//! `run` and `paragraph` are the resolved `RunProperties` and `ParagraphProperties`.

use super::{Reader, Writer};
use crate::editor::StyleEntry;
use cdoc_crdt::DecodeError;
use cdoc_model::{StyleId, StyleKind};

const MAGIC: &[u8; 4] = b"CDS\x01";
const KINDS: [StyleKind; 3] = [StyleKind::Paragraph, StyleKind::Character, StyleKind::Table];

/// The binary form of the style list.
#[must_use]
pub fn encode_styles(styles: &[StyleEntry]) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.usize(styles.len());
    for s in styles {
        w.str(s.id.as_str());
        w.str(&s.name);
        w.u8(match s.kind {
            StyleKind::Paragraph => 0,
            StyleKind::Character => 1,
            StyleKind::Table => 2,
        });
        match &s.based_on {
            Some(id) => {
                w.u8(1);
                w.str(id.as_str());
            }
            None => w.u8(0),
        }
        value!(w, &s.run);
        value!(w, &s.paragraph);
    }
    w.finish()
}

/// Reads [`encode_styles`]'s output.
pub fn decode_styles(bytes: &[u8]) -> Result<Vec<StyleEntry>, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let n = r.usize()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let id = StyleId::new(r.str()?);
        let name = r.str()?.to_owned();
        let tag = r.u8()?;
        let kind = *KINDS.get(usize::from(tag)).ok_or(DecodeError::BadTag {
            what: "style kind",
            tag,
        })?;
        let based_on = match r.u8()? {
            0 => None,
            1 => Some(StyleId::new(r.str()?)),
            tag => {
                return Err(DecodeError::BadTag {
                    what: "based on",
                    tag,
                })
            }
        };
        out.push(StyleEntry {
            id,
            name,
            kind,
            based_on,
            run: read_value!(r),
            paragraph: read_value!(r),
        });
    }
    r.finish()?;
    Ok(out)
}

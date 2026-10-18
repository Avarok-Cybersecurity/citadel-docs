//! The result's story section (version 5): which story the selection is in.
//!
//! ```text
//! story := 0                                  the body
//!        | 1 section:varint kind:u8           a header
//!        | 2 section:varint kind:u8           a footer
//!        | 3 id:varint                        a footnote body
//! ```
//!
//! `kind` is 0 default, 1 first page, 2 even pages, as in `EditHeaderFooter`.

use super::tables::{code, pick, HEADER_KINDS};
use super::{Reader, Writer};
use crate::story::Story;
use cdoc_crdt::DecodeError;
use cdoc_model::FootnoteId;

pub(super) fn write(w: &mut Writer, story: Story) {
    match story {
        Story::Body => w.u8(0),
        Story::Header { section, kind } | Story::Footer { section, kind } => {
            w.u8(if matches!(story, Story::Header { .. }) {
                1
            } else {
                2
            });
            w.uvar(u64::from(section));
            w.u8(code(&HEADER_KINDS, &kind));
        }
        Story::Footnote(id) => {
            w.u8(3);
            w.uvar(u64::from(id.0));
        }
    }
}

fn u32_of(r: &mut Reader<'_>, what: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(r.uvar()?).map_err(|_| DecodeError::OutOfRange(what))
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<Story, DecodeError> {
    match r.u8()? {
        0 => Ok(Story::Body),
        tag @ (1 | 2) => {
            let section = u32_of(r, "section")?;
            let kind = pick(&HEADER_KINDS, r, "header kind")?;
            Ok(if tag == 1 {
                Story::Header { section, kind }
            } else {
                Story::Footer { section, kind }
            })
        }
        3 => Ok(Story::Footnote(FootnoteId(u32_of(r, "footnote id")?))),
        tag => Err(DecodeError::BadTag { what: "story", tag }),
    }
}

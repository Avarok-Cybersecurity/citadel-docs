//! Text reduced to what a face can draw.

use super::{Fonts, SOFT_HYPHEN};
use crate::ir::FaceId;

/// Text the face can draw, with every character it cannot replaced by a visible marker, and
/// how many were replaced, so the count can be reported rather than left for a reader to
/// notice; `drawn` is told, for each character, how many characters it is drawn as. A soft
/// hyphen is kept whatever the face maps: it is never drawn as itself (the shaper hides it; a
/// line broken there gets a real hyphen).
///
/// Plain ASCII marker: a non-ASCII one looked safe once and rendered as a euro sign, which is
/// exactly the silent mangling the marker exists to make visible.
pub fn sanitise(
    text: &str,
    fonts: &Fonts<'_>,
    face: FaceId,
    mut drawn: impl FnMut(usize),
) -> (String, usize) {
    const MARKER: &str = "[?]";
    let mut out = String::with_capacity(text.len());
    let mut replaced = 0;
    for ch in text.chars() {
        if ch == SOFT_HYPHEN || fonts.has_glyph(face, ch) {
            out.push(ch);
            drawn(1);
        } else {
            out.push_str(MARKER);
            drawn(MARKER.len());
            replaced += 1;
        }
    }
    (out, replaced)
}

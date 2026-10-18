//! Arbitrary glyph runs, drawing only glyphs their face's table lists (the format refers to
//! glyphs by their place in that table): runs of arbitrary values, which exercise every
//! exception column, and runs shaped like real text (one glyph per character at the strike's
//! base advance or one more, with occasional ligatures, kerning and offsets), which exercise
//! the predictions.

use cdoc_layout::FaceId;
use cdoc_paint::{FaceEntry, Glyph, Op, Rgba, RunHead};
use proptest::prelude::*;

fn rgba() -> impl Strategy<Value = Rgba> {
    any::<[u8; 4]>().prop_map(|[r, g, b, a]| Rgba { r, g, b, a })
}

fn head(face: u16, size: impl Strategy<Value = i32>) -> impl Strategy<Value = RunHead> {
    (size, rgba(), any::<i32>(), any::<i32>()).prop_map(move |(size, colour, x, y)| RunHead {
        face: FaceId(face),
        size,
        colour,
        x,
        y,
    })
}

fn text(pattern: &str) -> impl Strategy<Value = String> {
    proptest::string::string_regex(pattern).expect("regex")
}

/// Arbitrary values: clusters any sorted character boundaries, advances and offsets any `i32`.
fn arbitrary(face: u16, ids: Vec<u16>) -> BoxedStrategy<Op> {
    let most = if ids.is_empty() { 0..1 } else { 0..8 };
    let glyph = (
        any::<prop::sample::Index>(),
        any::<prop::sample::Index>(),
        any::<[i32; 3]>(),
    );
    (
        head(face, any::<i32>()),
        text(".{0,12}"),
        prop::collection::vec(glyph, most),
    )
        .prop_map(move |(head, text, g)| {
            let bounds: Vec<u32> = (0..=text.len())
                .filter(|b| text.is_char_boundary(*b))
                .map(|b| u32::try_from(b).expect("short text"))
                .collect();
            let mut clusters: Vec<u32> = g
                .iter()
                .map(|(_, i, _)| bounds[i.index(bounds.len())])
                .collect();
            clusters.sort_unstable();
            let glyphs = g
                .into_iter()
                .zip(clusters)
                .map(|((id, _, [advance, x_offset, y_offset]), cluster)| Glyph {
                    id: ids[id.index(ids.len())],
                    cluster,
                    advance,
                    x_offset,
                    y_offset,
                })
                .collect();
            Op::GlyphRun { head, text, glyphs }
        })
        .boxed()
}

/// Text-shaped: one glyph per character, each character always the same glyph, advances at
/// `advance · size / em` rounded down or up; a few glyphs swapped, merged into the previous
/// cluster, kerned or offset.
fn shaped(face: &FaceEntry, face_id: u16) -> BoxedStrategy<Op> {
    let table = face.glyphs.clone();
    let em = i64::from(face.units_per_em);
    let tweak = prop_oneof![
        8 => Just(0_u8),
        1 => Just(1),
        1 => Just(2),
        1 => Just(3),
        1 => Just(4),
    ];
    let glyphs = prop::collection::vec(
        (
            any::<bool>(),
            tweak,
            any::<prop::sample::Index>(),
            -40..40_i32,
        ),
        24,
    );
    (
        head(face_id, 1..20_000_i32),
        text("[a-eé€😀 ]{0,24}"),
        glyphs,
    )
        .prop_map(move |(head, text, tweaks)| {
            let mut glyphs: Vec<Glyph> = Vec::new();
            for ((at, ch), (up, tweak, pick, small)) in text.char_indices().zip(tweaks) {
                let mut m = table[usize::try_from(u32::from(ch)).expect("char") % table.len()];
                if tweak == 1 {
                    m = table[pick.index(table.len())];
                }
                let base = i64::from(m.advance) * i64::from(head.size) / em;
                let advance = i32::try_from(base).expect("small") + i32::from(up);
                let cluster = match (tweak, glyphs.last()) {
                    (2, Some(prev)) => prev.cluster,
                    _ => u32::try_from(at).expect("short"),
                };
                glyphs.push(Glyph {
                    id: m.id,
                    cluster,
                    advance: if tweak == 3 { advance + small } else { advance },
                    x_offset: if tweak == 4 { small } else { 0 },
                    y_offset: if tweak == 4 { -small } else { 0 },
                });
            }
            Op::GlyphRun { head, text, glyphs }
        })
        .boxed()
}

/// A run in one of `faces`, arbitrary or text-shaped.
pub fn glyph_run(faces: &[FaceEntry]) -> BoxedStrategy<Op> {
    let options: Vec<BoxedStrategy<Op>> = (0_u16..)
        .zip(faces)
        .flat_map(|(k, face)| {
            let ids: Vec<u16> = face.glyphs.iter().map(|g| g.id).collect();
            let mut each = vec![arbitrary(k, ids)];
            if !face.glyphs.is_empty() {
                each.push(shaped(face, k));
            }
            each
        })
        .collect();
    proptest::strategy::Union::new(options).boxed()
}

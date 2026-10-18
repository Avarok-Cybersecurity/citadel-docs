//! Arbitrary display lists, for the codec's round trip: every opcode, every value range the
//! format can carry (full `i32` coordinates, full `u16` glyph ids and advances, any em, any
//! alpha), text runs drawing only glyphs their face's table lists, and clip pushes balanced
//! by pops, as the format requires.

use cdoc_layout::{FaceId, FaceKey, Rect, Segment};
use cdoc_paint::{DisplayList, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead};
use proptest::prelude::*;

fn rect() -> impl Strategy<Value = Rect> {
    (any::<i32>(), any::<i32>(), any::<i32>(), any::<i32>()).prop_map(|(x, y, width, height)| {
        Rect {
            x,
            y,
            width,
            height,
        }
    })
}

fn rgba() -> impl Strategy<Value = Rgba> {
    any::<[u8; 4]>().prop_map(|[r, g, b, a]| Rgba { r, g, b, a })
}

fn segment() -> impl Strategy<Value = Segment> {
    prop_oneof![
        (any::<i32>(), any::<i32>()).prop_map(|(x, y)| Segment::Move(x, y)),
        (any::<i32>(), any::<i32>()).prop_map(|(x, y)| Segment::Line(x, y)),
        any::<[i32; 6]>().prop_map(Segment::Cubic),
        Just(Segment::Close),
    ]
}

fn head(faces: u16) -> impl Strategy<Value = RunHead> {
    (0..faces, any::<i32>(), rgba(), any::<i32>(), any::<i32>()).prop_map(
        |(face, size, colour, x, y)| RunHead {
            face: FaceId(face),
            size,
            colour,
            x,
            y,
        },
    )
}

fn text() -> impl Strategy<Value = String> {
    proptest::string::string_regex(".{0,12}").expect("regex")
}

fn op(faces: &[FaceEntry]) -> BoxedStrategy<Op> {
    let count = u16::try_from(faces.len()).expect("few faces");
    let segments = || prop::collection::vec(segment(), 0..6);
    prop_oneof![
        (rect(), rgba()).prop_map(|(rect, colour)| Op::FillRect { rect, colour }),
        (rect(), any::<i32>(), rgba()).prop_map(|(rect, width, colour)| Op::StrokeRect {
            rect,
            width,
            colour
        }),
        (any::<[i32; 5]>(), rgba()).prop_map(|([x1, y1, x2, y2, width], colour)| Op::Line {
            from: (x1, y1),
            to: (x2, y2),
            width,
            colour
        }),
        (segments(), rgba()).prop_map(|(segments, colour)| Op::FillPath { segments, colour }),
        (segments(), any::<i32>(), rgba()).prop_map(|(segments, width, colour)| {
            Op::StrokePath {
                segments,
                width,
                colour,
            }
        }),
        (
            head(count),
            text(),
            prop::collection::vec(
                (
                    any::<u16>(),
                    any::<prop::sample::Index>(),
                    any::<[i32; 3]>()
                ),
                0..8
            )
        )
            .prop_map(|(head, text, g)| {
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
                        id,
                        cluster,
                        advance,
                        x_offset,
                        y_offset,
                    })
                    .collect();
                Op::GlyphRun { head, text, glyphs }
            }),
        (rect(), text()).prop_map(|(rect, content_id)| Op::Image { rect, content_id }),
        (rect(), text()).prop_map(|(rect, href)| Op::Link { rect, href }),
        rect().prop_map(Op::ClipPush),
    ]
    .boxed()
}

/// Ops with every push closed: a pop after each push, and pops for whatever is still open
/// at the end.
fn balanced(ops: Vec<Op>, pops: &[bool]) -> Vec<Op> {
    let mut out = Vec::new();
    let mut open = 0_usize;
    for (k, op) in ops.into_iter().enumerate() {
        let push = matches!(op, Op::ClipPush(_));
        out.push(op);
        open += usize::from(push);
        if open > 0 && pops.get(k).copied().unwrap_or(false) {
            out.push(Op::ClipPop);
            open -= 1;
        }
    }
    out.extend(std::iter::repeat_n(Op::ClipPop, open));
    out
}

fn face() -> impl Strategy<Value = FaceEntry> {
    (
        (text(), any::<bool>(), any::<bool>()),
        1..=u16::MAX,
        prop::collection::btree_map(any::<u16>(), any::<u16>(), 0..12),
    )
        .prop_map(|((family, bold, italic), units_per_em, glyphs)| FaceEntry {
            key: FaceKey {
                family,
                bold,
                italic,
            },
            units_per_em,
            glyphs: glyphs
                .into_iter()
                .map(|(id, advance)| GlyphMetric { id, advance })
                .collect(),
        })
}

/// A whole list: one to three faces, up to four pages of up to twelve ops.
pub fn display_list() -> BoxedStrategy<DisplayList> {
    prop::collection::vec(face(), 1..=3)
        .prop_flat_map(|faces| {
            let page = (
                any::<u32>(),
                any::<i32>(),
                any::<i32>(),
                prop::collection::vec(op(&faces), 0..12),
                prop::collection::vec(any::<bool>(), 12),
            )
                .prop_map(|(number, width, height, ops, pops)| PageOps {
                    number,
                    width,
                    height,
                    ops: balanced(ops, &pops),
                });
            (Just(faces), prop::collection::vec(page, 0..4))
        })
        .prop_map(|(faces, pages)| DisplayList { faces, pages })
        .boxed()
}

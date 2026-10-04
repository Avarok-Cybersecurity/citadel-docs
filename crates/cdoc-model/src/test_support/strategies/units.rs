//! Strategies for units and identifiers.

use crate::ids::{CommentId, FieldId, RevisionId, StyleId};
use crate::units::{BorderWidth, Color, FontSize, Length, TableWidth};
use proptest::prelude::*;

/// Any length a document might hold, including negative indents. Whole twips, the resolution
/// every `.docx` length has, so an exporter can write it without rounding.
pub fn length() -> BoxedStrategy<Length> {
    (-4_000i32..40_000).prop_map(Length::from_twips).boxed()
}

/// A strictly positive length, in whole twips.
pub fn positive_length() -> BoxedStrategy<Length> {
    (1i32..40_000).prop_map(Length::from_twips).boxed()
}

/// A length of zero or more, in whole twips: spacing, padding and hanging amounts, which
/// formats store unsigned.
pub fn non_negative_length() -> BoxedStrategy<Length> {
    (0i32..40_000).prop_map(Length::from_twips).boxed()
}

/// A whole number of points: border spacing, which formats store in points.
pub fn whole_points() -> BoxedStrategy<Length> {
    (0i32..32).prop_map(Length::from_points).boxed()
}

/// A usable font size.
pub fn font_size() -> BoxedStrategy<FontSize> {
    (1u16..200).prop_map(FontSize).boxed()
}

/// A border width.
pub fn border_width() -> BoxedStrategy<BorderWidth> {
    (0u16..64).prop_map(BorderWidth).boxed()
}

/// Any colour.
pub fn color() -> BoxedStrategy<Color> {
    (any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b)| Color::rgb(r, g, b))
        .boxed()
}

/// A table width of every kind.
pub fn table_width() -> BoxedStrategy<TableWidth> {
    prop_oneof![
        Just(TableWidth::Auto),
        positive_length().prop_map(|length| TableWidth::Fixed { length }),
        (1u16..=5000).prop_map(|fiftieths| TableWidth::Percent { fiftieths }),
    ]
    .boxed()
}

/// Non-empty text with letters, punctuation, a non-ASCII letter and an emoji, so JSON escaping
/// and UTF-8 boundaries are exercised.
///
/// # Panics
/// Never: the regex is a constant.
pub fn short_text() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-zA-Z0-9 ,.!?'\"<>&é😀-]{1,12}")
        .expect("valid regex")
        .boxed()
}

/// A short identifier.
///
/// # Panics
/// Never: the regex is a constant.
pub fn ident() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-z][a-z0-9-]{0,7}")
        .expect("valid regex")
        .boxed()
}

/// A style id.
pub fn style_id() -> BoxedStrategy<StyleId> {
    ident().prop_map(StyleId).boxed()
}

/// A comment id.
pub fn comment_id() -> BoxedStrategy<CommentId> {
    ident().prop_map(CommentId).boxed()
}

/// A field id.
pub fn field_id() -> BoxedStrategy<FieldId> {
    ident().prop_map(FieldId).boxed()
}

/// A revision id.
pub fn revision_id() -> BoxedStrategy<RevisionId> {
    ident().prop_map(RevisionId).boxed()
}

/// An RFC 3339 timestamp.
pub fn rfc3339() -> BoxedStrategy<String> {
    (2000u16..2100, 1u8..=12, 1u8..=28, 0u8..24, 0u8..60)
        .prop_map(|(y, mo, d, h, mi)| format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:00Z"))
        .boxed()
}

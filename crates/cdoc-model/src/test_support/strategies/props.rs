//! Strategies for run, paragraph, border and shading properties.

use super::units::{
    border_width, color, font_size, length, non_negative_length, positive_length, style_id,
    whole_points,
};
use super::{opt, pick};
use crate::borders::{Border, BorderStyle, Borders, Shading};
use crate::ids::NumId;
use crate::paragraph::{
    Alignment, FirstLine, Indent, LineSpacing, NumberingRef, ParagraphProperties, Spacing,
};
use crate::run::{RunProperties, Underline, UnderlineStyle, VerticalAlign};
use proptest::prelude::*;

const BORDER_STYLES: &[BorderStyle] = &[
    BorderStyle::None,
    BorderStyle::Single,
    BorderStyle::Thick,
    BorderStyle::Double,
    BorderStyle::Dotted,
    BorderStyle::Dashed,
    BorderStyle::DotDash,
    BorderStyle::Triple,
];
const UNDERLINES: &[UnderlineStyle] = &[
    UnderlineStyle::None,
    UnderlineStyle::Single,
    UnderlineStyle::Double,
    UnderlineStyle::Thick,
    UnderlineStyle::Dotted,
    UnderlineStyle::Dashed,
    UnderlineStyle::Wave,
    UnderlineStyle::Words,
];
const VERTICAL: &[VerticalAlign] = &[
    VerticalAlign::Baseline,
    VerticalAlign::Superscript,
    VerticalAlign::Subscript,
];
/// Every alignment.
pub const ALIGNMENTS: &[Alignment] = &[
    Alignment::Left,
    Alignment::Center,
    Alignment::Right,
    Alignment::Justify,
];

/// One border edge.
pub fn border() -> BoxedStrategy<Border> {
    (
        pick(BORDER_STYLES),
        border_width(),
        opt(color()),
        whole_points(),
    )
        .prop_map(|(style, width, color, space)| Border {
            style,
            width,
            color,
            space,
        })
        .boxed()
}

/// A border set with each edge independently present or absent.
pub fn borders() -> BoxedStrategy<Borders> {
    (
        opt(border()),
        opt(border()),
        opt(border()),
        opt(border()),
        opt(border()),
    )
        .prop_map(|(top, bottom, left, right, between)| Borders {
            top,
            bottom,
            left,
            right,
            between,
        })
        .boxed()
}

/// Shading.
pub fn shading() -> BoxedStrategy<Shading> {
    color().prop_map(|fill| Shading { fill }).boxed()
}

/// An underline.
pub fn underline() -> BoxedStrategy<Underline> {
    (pick(UNDERLINES), opt(color()))
        .prop_map(|(style, color)| Underline { style, color })
        .boxed()
}

/// Run properties with every field independently present or absent.
pub fn run_properties() -> BoxedStrategy<RunProperties> {
    let a = (
        opt(style_id()),
        opt(pick(&["Calibri", "Times New Roman", "Courier New"]).prop_map(str::to_owned)),
        opt(font_size()),
        opt(any::<bool>()),
        opt(any::<bool>()),
        opt(underline()),
    );
    let b = (
        opt(any::<bool>()),
        opt(pick(VERTICAL)),
        opt(color()),
        opt(color()),
        opt(any::<bool>()),
        opt(any::<bool>()),
    );
    (a, b)
        .prop_map(
            |(
                (style, font, size, bold, italic, underline),
                (strike, vertical_align, color, highlight, small_caps, all_caps),
            )| RunProperties {
                style,
                font,
                size,
                bold,
                italic,
                underline,
                strike,
                vertical_align,
                color,
                highlight,
                small_caps,
                all_caps,
            },
        )
        .boxed()
}

/// Line spacing of every rule.
pub fn line_spacing() -> BoxedStrategy<LineSpacing> {
    prop_oneof![
        (1u32..1000).prop_map(|lines| LineSpacing::Multiple { lines }),
        positive_length().prop_map(|height| LineSpacing::Exact { height }),
        positive_length().prop_map(|height| LineSpacing::AtLeast { height }),
    ]
    .boxed()
}

/// Spacing.
pub fn spacing() -> BoxedStrategy<Spacing> {
    (
        opt(non_negative_length()),
        opt(non_negative_length()),
        opt(line_spacing()),
    )
        .prop_map(|(before, after, line)| Spacing {
            before,
            after,
            line,
        })
        .boxed()
}

/// Indentation.
pub fn indent() -> BoxedStrategy<Indent> {
    let first = prop_oneof![
        non_negative_length().prop_map(|by| FirstLine::Indent { by }),
        non_negative_length().prop_map(|by| FirstLine::Hanging { by }),
    ];
    (opt(length()), opt(length()), opt(first))
        .prop_map(|(left, right, first_line)| Indent {
            left,
            right,
            first_line,
        })
        .boxed()
}

/// A list membership.
pub fn numbering_ref() -> BoxedStrategy<NumberingRef> {
    (1u32..10, 0u8..=8)
        .prop_map(|(n, level)| NumberingRef {
            num_id: NumId(n),
            level,
        })
        .boxed()
}

/// Paragraph properties with every field independently present or absent.
pub fn paragraph_properties() -> BoxedStrategy<ParagraphProperties> {
    let a = (
        opt(style_id()),
        opt(pick(ALIGNMENTS)),
        opt(spacing()),
        opt(indent()),
        opt(borders()),
        opt(shading()),
    );
    let b = (
        opt(any::<bool>()),
        opt(any::<bool>()),
        opt(any::<bool>()),
        opt(numbering_ref()),
        opt(0u8..=8),
    );
    (a, b)
        .prop_map(
            |(
                (style, alignment, spacing, indent, borders, shading),
                (keep_with_next, keep_lines, page_break_before, numbering, outline_level),
            )| ParagraphProperties {
                style,
                alignment,
                spacing,
                indent,
                borders,
                shading,
                keep_with_next,
                keep_lines,
                page_break_before,
                numbering,
                outline_level,
            },
        )
        .boxed()
}

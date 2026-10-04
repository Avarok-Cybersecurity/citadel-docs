//! One table per enum: the OOXML spelling of each variant. Both directions come from the same
//! table, so they cannot disagree. Aliases are spellings only the reader accepts.

use cdoc_model::{
    Alignment, BorderStyle, HeaderFooterKind, NumberFormat, Orientation, SectionStart, StyleKind,
    UnderlineStyle, VerticalAlign, VerticalCellAlign,
};

macro_rules! names {
    ($to:ident, $from:ident, $ty:ty, { $($variant:path => $text:literal),+ $(,)? }
     $(, aliases { $($alias:literal => $target:path),+ $(,)? })?) => {
        #[doc = concat!("The OOXML name of a [`", stringify!($ty), "`].")]
        #[must_use]
        pub const fn $to(value: $ty) -> &'static str {
            match value { $($variant => $text),+ }
        }

        #[doc = concat!("The [`", stringify!($ty), "`] an OOXML name stands for.")]
        #[must_use]
        pub fn $from(text: &str) -> Option<$ty> {
            match text {
                $($text => Some($variant),)+
                $($($alias => Some($target),)+)?
                _ => None,
            }
        }
    };
}

names!(alignment_name, alignment_from, Alignment, {
    Alignment::Left => "left",
    Alignment::Center => "center",
    Alignment::Right => "right",
    Alignment::Justify => "both",
}, aliases {
    "start" => Alignment::Left,
    "end" => Alignment::Right,
    "justify" => Alignment::Justify,
});

names!(border_style_name, border_style_from, BorderStyle, {
    BorderStyle::None => "none",
    BorderStyle::Single => "single",
    BorderStyle::Thick => "thick",
    BorderStyle::Double => "double",
    BorderStyle::Dotted => "dotted",
    BorderStyle::Dashed => "dashed",
    BorderStyle::DotDash => "dotDash",
    BorderStyle::Triple => "triple",
}, aliases { "nil" => BorderStyle::None });

names!(underline_name, underline_from, UnderlineStyle, {
    UnderlineStyle::None => "none",
    UnderlineStyle::Single => "single",
    UnderlineStyle::Double => "double",
    UnderlineStyle::Thick => "thick",
    UnderlineStyle::Dotted => "dotted",
    UnderlineStyle::Dashed => "dash",
    UnderlineStyle::Wave => "wave",
    UnderlineStyle::Words => "words",
});

names!(vertical_align_name, vertical_align_from, VerticalAlign, {
    VerticalAlign::Baseline => "baseline",
    VerticalAlign::Superscript => "superscript",
    VerticalAlign::Subscript => "subscript",
});

names!(number_format_name, number_format_from, NumberFormat, {
    NumberFormat::Decimal => "decimal",
    NumberFormat::LowerLetter => "lowerLetter",
    NumberFormat::UpperLetter => "upperLetter",
    NumberFormat::LowerRoman => "lowerRoman",
    NumberFormat::UpperRoman => "upperRoman",
    NumberFormat::Bullet => "bullet",
    NumberFormat::None => "none",
});

names!(orientation_name, orientation_from, Orientation, {
    Orientation::Portrait => "portrait",
    Orientation::Landscape => "landscape",
});

names!(section_start_name, section_start_from, SectionStart, {
    SectionStart::NextPage => "nextPage",
    SectionStart::Continuous => "continuous",
    SectionStart::EvenPage => "evenPage",
    SectionStart::OddPage => "oddPage",
});

names!(header_kind_name, header_kind_from, HeaderFooterKind, {
    HeaderFooterKind::Default => "default",
    HeaderFooterKind::First => "first",
    HeaderFooterKind::Even => "even",
});

names!(cell_align_name, cell_align_from, VerticalCellAlign, {
    VerticalCellAlign::Top => "top",
    VerticalCellAlign::Center => "center",
    VerticalCellAlign::Bottom => "bottom",
});

names!(style_kind_name, style_kind_from, StyleKind, {
    StyleKind::Paragraph => "paragraph",
    StyleKind::Character => "character",
    StyleKind::Table => "table",
});

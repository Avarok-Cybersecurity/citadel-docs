//! Character formatting Markdown has no syntax for.

use super::pieces::MONOSPACE;
use super::Ctx;
use cdoc_model::{RunProperties, VerticalAlign};

pub(super) fn note_formatting(props: &RunProperties, ctx: &mut Ctx<'_>) {
    let dropped = [
        (props.underline.is_some(), "underline"),
        (props.color.is_some(), "text colour"),
        (props.highlight.is_some(), "highlight"),
        (props.size.is_some(), "font size"),
        (
            props
                .vertical_align
                .is_some_and(|v| v != VerticalAlign::Baseline),
            "superscript or subscript",
        ),
        (
            props.small_caps == Some(true) || props.all_caps == Some(true),
            "small or all capitals",
        ),
    ];
    for (present, what) in dropped {
        if present {
            ctx.lose(what);
        }
    }
    if props
        .font
        .as_ref()
        .is_some_and(|f| !MONOSPACE.contains(&f.as_str()))
    {
        ctx.lose("font family");
    }
}

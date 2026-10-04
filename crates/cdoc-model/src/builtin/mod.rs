//! The built-in style sheet: Word's default template (Calibri 11, Calibri Light headings in
//! the blue accent), so a new Citadel document looks like a new Word document and the styles
//! gallery has the names users expect.

pub mod ids;

use crate::ids::StyleId;
use crate::paragraph::{Alignment, Indent, LineSpacing, ParagraphProperties, Spacing};
use crate::run::{RunProperties, Underline, UnderlineStyle};
use crate::style::{DocumentDefaults, Style, StyleKind, StyleSheet};
use crate::units::{Color, FontSize, Length};

const ACCENT: Color = Color::rgb(0x2F, 0x54, 0x96);
const ACCENT_DARK: Color = Color::rgb(0x1F, 0x37, 0x63);
const QUOTE_GREY: Color = Color::rgb(0x40, 0x40, 0x40);
const LINK_BLUE: Color = Color::rgb(0x05, 0x63, 0xC1);
const SINGLE: LineSpacing = LineSpacing::Multiple { lines: 240 };

fn spacing(before_twips: i32, after_twips: i32, line: Option<LineSpacing>) -> Spacing {
    Spacing {
        before: Some(Length::from_twips(before_twips)),
        after: Some(Length::from_twips(after_twips)),
        line,
    }
}

fn light(size_pt: u16) -> RunProperties {
    RunProperties {
        font: Some("Calibri Light".to_owned()),
        size: Some(FontSize::from_points(size_pt)),
        ..RunProperties::none()
    }
}

fn paragraph_style(
    id: StyleId,
    name: &str,
    paragraph: ParagraphProperties,
    run: RunProperties,
) -> Style {
    let based_on = (id != ids::normal()).then(ids::normal);
    Style {
        id,
        name: name.to_owned(),
        kind: StyleKind::Paragraph,
        based_on,
        next: Some(ids::normal()),
        paragraph,
        run,
    }
}

fn heading(level: u8, size_pt: u16, color: Color, italic: bool) -> Style {
    let id = ids::heading(level).expect("level is 1..=6");
    let paragraph = ParagraphProperties {
        keep_with_next: Some(true),
        keep_lines: Some(true),
        spacing: Some(spacing(if level == 1 { 240 } else { 40 }, 0, None)),
        outline_level: Some(level - 1),
        ..ParagraphProperties::none()
    };
    let run = RunProperties {
        color: Some(color),
        italic: italic.then_some(true),
        ..light(size_pt)
    };
    paragraph_style(id, &format!("heading {level}"), paragraph, run)
}

fn title() -> Style {
    let paragraph = ParagraphProperties {
        spacing: Some(spacing(0, 0, Some(SINGLE))),
        ..ParagraphProperties::none()
    };
    paragraph_style(ids::title(), "Title", paragraph, light(28))
}

fn quote() -> Style {
    let paragraph = ParagraphProperties {
        alignment: Some(Alignment::Center),
        spacing: Some(spacing(200, 280, None)),
        ..ParagraphProperties::none()
    };
    let run = RunProperties {
        italic: Some(true),
        color: Some(QUOTE_GREY),
        ..RunProperties::none()
    };
    paragraph_style(ids::quote(), "Quote", paragraph, run)
}

fn list_paragraph() -> Style {
    let paragraph = ParagraphProperties {
        indent: Some(Indent {
            left: Some(Length::from_twips(720)),
            right: None,
            first_line: None,
        }),
        ..ParagraphProperties::none()
    };
    paragraph_style(
        ids::list_paragraph(),
        "List Paragraph",
        paragraph,
        RunProperties::none(),
    )
}

/// Single-spaced, no space after: headers, footers and footnote text.
fn compact(id: StyleId, name: &str, run: RunProperties) -> Style {
    let paragraph = ParagraphProperties {
        spacing: Some(spacing(0, 0, Some(SINGLE))),
        ..ParagraphProperties::none()
    };
    paragraph_style(id, name, paragraph, run)
}

fn hyperlink() -> Style {
    Style {
        id: ids::hyperlink(),
        name: "Hyperlink".to_owned(),
        kind: StyleKind::Character,
        based_on: None,
        next: None,
        paragraph: ParagraphProperties::none(),
        run: RunProperties {
            color: Some(LINK_BLUE),
            underline: Some(Underline {
                style: UnderlineStyle::Single,
                color: None,
            }),
            ..RunProperties::none()
        },
    }
}

impl StyleSheet {
    /// Word's default template styles. A named preset the caller chooses explicitly.
    #[must_use]
    pub fn word_builtin() -> Self {
        let defaults = DocumentDefaults {
            run: RunProperties {
                font: Some("Calibri".to_owned()),
                size: Some(FontSize::from_points(11)),
                ..RunProperties::none()
            },
            paragraph: ParagraphProperties {
                spacing: Some(spacing(0, 160, Some(LineSpacing::Multiple { lines: 259 }))),
                ..ParagraphProperties::none()
            },
        };
        let footnote_run = RunProperties {
            size: Some(FontSize::from_points(10)),
            ..RunProperties::none()
        };
        Self {
            defaults,
            styles: vec![
                paragraph_style(
                    ids::normal(),
                    "Normal",
                    ParagraphProperties::none(),
                    RunProperties::none(),
                ),
                title(),
                heading(1, 16, ACCENT, false),
                heading(2, 13, ACCENT, false),
                heading(3, 12, ACCENT_DARK, false),
                heading(4, 11, ACCENT, true),
                heading(5, 11, ACCENT, false),
                heading(6, 11, ACCENT_DARK, false),
                quote(),
                list_paragraph(),
                compact(ids::footnote_text(), "footnote text", footnote_run),
                compact(ids::header(), "header", RunProperties::none()),
                compact(ids::footer(), "footer", RunProperties::none()),
                hyperlink(),
            ],
        }
    }
}

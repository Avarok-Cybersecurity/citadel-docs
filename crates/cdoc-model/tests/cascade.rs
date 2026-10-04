//! The style cascade: defaults, then the paragraph style chain, then the character style,
//! then direct formatting; cycles and unknown styles are errors, not panics or silent skips.

use cdoc_model::builtin::ids;
use cdoc_model::{
    Color, FontSize, ParagraphProperties, RunProperties, Style, StyleError, StyleId, StyleKind,
    StyleSheet, VerticalAlign,
};

fn heading1_paragraph() -> ParagraphProperties {
    ParagraphProperties {
        style: ids::heading(1),
        ..ParagraphProperties::none()
    }
}

#[test]
fn heading_inherits_defaults_and_overrides_font() {
    let sheet = StyleSheet::word_builtin();
    let run = sheet
        .resolve_run(&heading1_paragraph(), &RunProperties::none())
        .unwrap();
    assert_eq!(run.font.as_deref(), Some("Calibri Light"));
    assert_eq!(run.size, Some(FontSize::from_points(16)));
    assert_eq!(run.color, Some(Color::rgb(0x2F, 0x54, 0x96)));
    assert_eq!(run.bold, None, "Word's Heading 1 is not bold");

    let paragraph = sheet.resolve_paragraph(&heading1_paragraph()).unwrap();
    assert_eq!(paragraph.keep_with_next, Some(true));
    assert_eq!(paragraph.outline_level, Some(0));
    assert!(
        paragraph.spacing.unwrap().after.is_some(),
        "space-after comes from the Normal chain"
    );
}

#[test]
fn direct_formatting_wins_over_character_style_over_paragraph_style() {
    let sheet = StyleSheet::word_builtin();
    let direct = RunProperties {
        style: Some(ids::hyperlink()),
        color: Some(Color::rgb(1, 2, 3)),
        ..RunProperties::none()
    };
    let run = sheet.resolve_run(&heading1_paragraph(), &direct).unwrap();
    assert_eq!(
        run.color,
        Some(Color::rgb(1, 2, 3)),
        "direct beats Hyperlink's blue"
    );
    assert!(run.underline.is_some(), "Hyperlink's underline survives");
    assert_eq!(
        run.font.as_deref(),
        Some("Calibri Light"),
        "Heading 1's font survives"
    );
}

#[test]
fn explicit_false_overrides_inherited_true() {
    let sheet = StyleSheet::word_builtin();
    let quote = ParagraphProperties {
        style: Some(ids::quote()),
        ..ParagraphProperties::none()
    };
    let direct = RunProperties {
        italic: Some(false),
        vertical_align: Some(VerticalAlign::Baseline),
        ..RunProperties::none()
    };
    let run = sheet.resolve_run(&quote, &direct).unwrap();
    assert_eq!(run.italic, Some(false));
    assert_eq!(run.vertical_align, Some(VerticalAlign::Baseline));
}

#[test]
fn unknown_style_is_an_error() {
    let sheet = StyleSheet::word_builtin();
    let props = ParagraphProperties {
        style: Some(StyleId::new("Nope")),
        ..ParagraphProperties::none()
    };
    assert_eq!(
        sheet.resolve_paragraph(&props),
        Err(StyleError::Unknown(StyleId::new("Nope")))
    );
}

#[test]
fn based_on_cycle_is_detected_not_looped() {
    let mut sheet = StyleSheet::word_builtin();
    let a = StyleId::new("A");
    let b = StyleId::new("B");
    for (id, base) in [(&a, &b), (&b, &a)] {
        sheet.styles.push(Style {
            id: id.clone(),
            name: id.as_str().to_owned(),
            kind: StyleKind::Paragraph,
            based_on: Some(base.clone()),
            next: None,
            paragraph: ParagraphProperties::none(),
            run: RunProperties::none(),
        });
    }
    assert_eq!(sheet.chain(&a), Err(StyleError::Cycle(a.clone())));
    let props = ParagraphProperties {
        style: Some(a.clone()),
        ..ParagraphProperties::none()
    };
    assert_eq!(sheet.resolve_paragraph(&props), Err(StyleError::Cycle(a)));
}

#[test]
fn builtin_sheet_has_the_gallery_names_and_resolves_every_style() {
    let sheet = StyleSheet::word_builtin();
    for id in [ids::normal(), ids::title(), ids::quote(), ids::hyperlink()] {
        assert!(sheet.get(&id).is_some(), "{id:?} missing");
    }
    for level in 1..=6 {
        assert!(sheet.get(&ids::heading(level).unwrap()).is_some());
    }
    assert_eq!(ids::heading(0), None);
    assert_eq!(ids::heading(7), None);
    for style in &sheet.styles {
        sheet.chain(&style.id).unwrap();
    }
}

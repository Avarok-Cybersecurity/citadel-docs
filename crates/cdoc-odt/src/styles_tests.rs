use super::*;
use crate::residual::Residual;
use cdoc_model::test_support::strategies::style_sheet;
use cdoc_model::{Alignment, Color, FontSize};
use proptest::prelude::*;

fn sheet(styles: Vec<Style>) -> StyleSheet {
    StyleSheet {
        defaults: StyleSheet::word_builtin().defaults,
        styles,
    }
}

fn read_back(style: &Style, names: &StyleNames) -> Style {
    let mut log = Log::new("t");
    read_style(&write_style(style, names, None), &BTreeMap::new(), &mut log)
        .expect("a model family")
}

#[test]
fn a_paragraph_style_reads_back_exactly() {
    let style = Style {
        id: StyleId::new("Heading1"),
        name: "Heading 1".to_owned(),
        kind: StyleKind::Paragraph,
        based_on: Some(StyleId::new("Normal")),
        next: Some(StyleId::new("Normal")),
        paragraph: ParagraphProperties {
            alignment: Some(Alignment::Center),
            keep_with_next: Some(true),
            outline_level: Some(0),
            ..ParagraphProperties::none()
        },
        run: RunProperties {
            bold: Some(true),
            size: Some(FontSize(32)),
            color: Some(Color::rgb(0x2F, 0x54, 0x96)),
            ..RunProperties::none()
        },
    };
    let names = StyleNames::for_sheet(&sheet(vec![style.clone()]));
    assert_eq!(read_back(&style, &names), style);
}

#[test]
fn ids_that_are_not_xml_names_are_escaped_and_kept_unique() {
    let style = |id: &str| Style {
        id: StyleId::new(id),
        name: id.to_owned(),
        kind: StyleKind::Character,
        based_on: None,
        next: None,
        paragraph: ParagraphProperties::none(),
        run: RunProperties::none(),
    };
    let names = StyleNames::for_sheet(&sheet(vec![
        style("Heading 1"),
        style("Heading_20_1"),
        style("1st"),
    ]));
    assert_eq!(names.of(&StyleId::new("Heading 1")), "Heading_20_1");
    assert_eq!(names.of(&StyleId::new("Heading_20_1")), "Heading_20_1_2");
    assert_eq!(names.of(&StyleId::new("1st")), "_31_st");
    assert_eq!(names.of(&StyleId::new("undefined id")), "undefined_20_id");
}

#[test]
fn defaults_and_the_tab_stop_read_back() {
    let defaults = StyleSheet::word_builtin().defaults;
    let mut log = Log::new("t");
    let (back, tab) = read_defaults(
        &write_defaults(&defaults, Length(1440)),
        &BTreeMap::new(),
        &mut log,
    );
    assert_eq!((back, tab), (defaults, Some(Length(1440))));
    assert!(log.report.is_lossless(), "{:?}", log.report);
}

/// Where a generated style may legitimately differ from what ODF holds: the halves a family
/// has no element for, `next` outside paragraph styles, and the paragraph values ODF lacks.
fn has_no_slot(place: &str, kind: StyleKind) -> bool {
    let paragraph_gaps = [
        "/paragraph/borders/between",
        "/paragraph/spacing",
        "/paragraph/indent",
        "/paragraph/borders",
        "/paragraph/numbering",
        "/paragraph/style",
    ];
    let border_style = place.starts_with("/paragraph/borders/") && place.ends_with("/style");
    if place == "/run/style" {
        return true;
    }
    match kind {
        StyleKind::Paragraph => paragraph_gaps.contains(&place) || border_style,
        StyleKind::Character => place == "/next" || place.starts_with("/paragraph"),
        StyleKind::Table => {
            place == "/next" || place.starts_with("/paragraph") || place.starts_with("/run")
        }
    }
}

proptest! {
    #[test]
    fn generated_styles_differ_only_where_odf_has_no_slot(sheet in style_sheet()) {
        let names = StyleNames::for_sheet(&sheet);
        for style in &sheet.styles {
            let back = read_back(style, &names);
            let residual = Residual::between(style, &back, &[]);
            for place in residual.places() {
                prop_assert!(has_no_slot(&place, style.kind), "{place} lost for {:?}", style.kind);
            }
            prop_assert_eq!(residual.apply(&back), Ok(style.clone()));
        }
    }
}

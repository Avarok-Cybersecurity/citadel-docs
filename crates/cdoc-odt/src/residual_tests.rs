use super::*;
use cdoc_model::test_support::strategies::paragraph_properties;
use cdoc_model::{Border, BorderStyle, BorderWidth, Borders, Length, ParagraphProperties};
use proptest::prelude::*;

fn border(style: BorderStyle) -> Border {
    Border {
        style,
        width: BorderWidth(4),
        color: None,
        space: Length(0),
    }
}

#[test]
fn only_the_differing_leaves_are_recorded_and_applying_them_restores_the_model() {
    let model = ParagraphProperties {
        keep_lines: Some(true),
        borders: Some(Borders {
            top: Some(border(BorderStyle::Triple)),
            between: Some(border(BorderStyle::Single)),
            ..Borders::none()
        }),
        ..ParagraphProperties::none()
    };
    let native = ParagraphProperties {
        keep_lines: Some(true),
        borders: Some(Borders {
            top: Some(border(BorderStyle::Double)),
            ..Borders::none()
        }),
        outline_level: Some(3),
        ..ParagraphProperties::none()
    };
    let residual = Residual::between(&model, &native, &[]);
    let places: Vec<String> = residual.places().collect();
    assert_eq!(
        places,
        ["/borders/between", "/borders/top/style", "/outlineLevel"]
    );
    assert_eq!(residual.apply(&native), Ok(model.clone()));
    let stored = Residual::from_json(&residual.to_json()).expect("parses");
    assert_eq!(stored.apply(&native), Ok(model));
}

#[test]
fn skipped_fields_are_never_corrected() {
    let model = ParagraphProperties {
        style: Some(cdoc_model::StyleId::new("a")),
        ..ParagraphProperties::none()
    };
    let residual = Residual::between(&model, &ParagraphProperties::none(), &["style"]);
    assert!(residual.is_empty());
}

#[test]
fn array_elements_are_corrected_in_place_and_reported_with_a_star() {
    let model = vec![Length(1), Length(2), Length(3)];
    let native = vec![Length(1), Length(9), Length(3)];
    let residual = Residual::between(&model, &native, &[]);
    assert_eq!(residual.places().collect::<Vec<_>>(), ["/*"]);
    assert_eq!(residual.apply(&native), Ok(model));
    let shorter = vec![Length(1)];
    let whole = Residual::between(&vec![Length(1), Length(2)], &shorter, &[]);
    assert_eq!(whole.places().collect::<Vec<_>>(), [""]);
    assert_eq!(whole.apply(&shorter), Ok(vec![Length(1), Length(2)]));
}

#[test]
fn damaged_residuals_are_errors() {
    assert!(Residual::from_json("[1]").is_err());
    assert!(Residual::from_json("{").is_err());
    let bad = Residual::from_json(r#"{"/keepLines":"yes"}"#).expect("parses");
    assert!(bad.apply(&ParagraphProperties::none()).is_err());
}

proptest! {
    #[test]
    fn any_two_values_differ_by_exactly_their_residual(
        model in paragraph_properties(),
        native in paragraph_properties(),
    ) {
        let residual = Residual::between(&model, &native, &[]);
        prop_assert_eq!(residual.apply(&native), Ok(model.clone()));
        prop_assert_eq!(residual.is_empty(), model == native);
    }
}

//! Units convert exactly and serialise as the bare number or `#RRGGBB`.

use cdoc_model::{Color, FontSize, Length, TableWidth};

#[test]
fn twips_points_and_inches_are_exact() {
    assert_eq!(Length::from_twips(1440), Length(7200));
    assert_eq!(Length::from_points(12), Length(1200));
    assert_eq!(Length::from_inches(1.0), Length(7200));
    assert_eq!(Length::from_inches(0.5), Length(3600));
    assert_eq!(Length::from_mm(25.4), Length(7200));
    assert_eq!(Length::from_twips(720).to_points(), 36.0);
}

#[test]
fn length_sign_predicates() {
    assert!(Length(0).is_non_negative());
    assert!(!Length(0).is_positive());
    assert!(!Length(-1).is_non_negative());
    assert!(Length(1).is_positive());
}

#[test]
fn font_size_is_half_points() {
    assert_eq!(FontSize::from_points(11), FontSize(22));
    assert_eq!(FontSize(23).to_points(), 11.5);
    assert!(!FontSize(0).is_valid());
}

#[test]
fn color_hex_round_trips_and_rejects_garbage() {
    let c = Color::rgb(0x2F, 0x54, 0x96);
    assert_eq!(c.hex(), "#2F5496");
    assert_eq!(Color::parse_hex("#2f5496"), Ok(c));
    assert_eq!(Color::parse_hex("2F5496"), Ok(c));
    assert!(Color::parse_hex("#2F549").is_err());
    assert!(Color::parse_hex("#GGGGGG").is_err());
    assert!(Color::parse_hex("#2F54961").is_err());
    assert!(Color::parse_hex("#ééé").is_err());
}

#[test]
fn units_serialise_as_scalars_and_tagged_objects() {
    assert_eq!(serde_json::to_string(&Length(7200)).unwrap(), "7200");
    assert_eq!(serde_json::to_string(&FontSize(22)).unwrap(), "22");
    assert_eq!(
        serde_json::to_string(&Color::rgb(1, 2, 255)).unwrap(),
        "\"#0102FF\""
    );
    let pct = TableWidth::Percent { fiftieths: 2500 };
    let json = serde_json::to_string(&pct).unwrap();
    assert_eq!(json, r#"{"kind":"percent","fiftieths":2500}"#);
    assert_eq!(serde_json::from_str::<TableWidth>(&json).unwrap(), pct);
    assert!(serde_json::from_str::<Color>("\"red\"").is_err());
}

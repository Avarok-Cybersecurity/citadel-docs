use super::*;

#[test]
fn every_unit_converts_exactly_then_rounds_once_half_away_from_zero() {
    assert_eq!(length("2.54cm"), Some(7200));
    assert_eq!(length("1in"), Some(7200));
    assert_eq!(length("1inch"), Some(7200));
    assert_eq!(length("25.4mm"), Some(7200));
    assert_eq!(length("72pt"), Some(7200));
    assert_eq!(length("6pc"), Some(7200));
    assert_eq!(length("96px"), Some(7200));
    assert_eq!(length("1cm"), Some(2835), "2834.6 rounds up");
    assert_eq!(length("0.005pt"), Some(1), "a half rounds away from zero");
    assert_eq!(length("-0.005pt"), Some(-1));
    assert_eq!(length("0.004pt"), Some(0));
    assert_eq!(length(" 12pt "), Some(1200));
    assert_eq!(length(".5pt"), Some(50));
}

#[test]
fn malformed_lengths_are_none() {
    for bad in [
        "", "pt", "12", "12em", "1.2.3pt", "--1pt", "1e3pt", "12 pt x",
    ] {
        assert_eq!(length(bad), None, "{bad:?}");
    }
}

#[test]
fn written_lengths_read_back_exactly() {
    for v in [-40_000, -5, -1, 0, 1, 5, 50, 99, 100, 1234, 7200, 2_000_000] {
        assert_eq!(length(&write_length(v)), Some(v), "{v}");
    }
    assert_eq!(write_length(1250), "12.5pt");
    assert_eq!(write_length(1205), "12.05pt");
    assert_eq!(write_length(-5), "-0.05pt");
    assert_eq!(write_length(0), "0pt");
}

#[test]
fn a_negative_zero_survives_for_the_hang() {
    assert_eq!(write_signed_length(0, true), "-0pt");
    assert_eq!(signed_length("-0pt"), Some((0, true)));
    assert_eq!(signed_length("0pt"), Some((0, false)));
    assert_eq!(
        signed_length(&write_signed_length(360, true)),
        Some((-360, true))
    );
}

#[test]
fn font_sizes_and_border_widths_are_exact_in_their_units() {
    for hp in [1u16, 21, 22, 23, 199] {
        assert_eq!(half_points(&write_half_points(hp)), Some(hp));
    }
    assert_eq!(write_half_points(23), "11.5pt");
    assert_eq!(
        half_points("11.3pt"),
        Some(23),
        "22.6 half-points round to 23"
    );
    for e in [0u16, 1, 3, 5, 63] {
        assert_eq!(eighths(&write_eighths(e)), Some(e));
    }
    assert_eq!(write_eighths(3), "0.375pt");
    assert_eq!(
        eighths("0.05pt"),
        Some(1),
        "a hairline is the thinnest line, not none"
    );
    assert_eq!(eighths("0pt"), Some(0));
}

#[test]
fn percentages_round_trip_in_the_models_units() {
    for f in [1u16, 1666, 2500, 5000] {
        assert_eq!(fiftieths(&write_fiftieths(f)), Some(f));
    }
    assert_eq!(write_fiftieths(2500), "50%");
    assert_eq!(fiftieths("33.33%"), Some(1667));
    for lines in 1u32..1000 {
        assert_eq!(
            line_multiple(&write_line_multiple(lines)),
            Some(lines),
            "{lines}"
        );
    }
    assert_eq!(write_line_multiple(276), "115%");
    assert_eq!(write_line_multiple(1), "0.4167%");
    assert_eq!(line_multiple("150%"), Some(360));
}

use super::*;
use cdoc_model::{AbstractNumId, Length};
use cdoc_model::{Color, RunProperties};
use cdoc_pkg::Log;
use std::collections::BTreeMap;

fn level(format: NumberFormat, text: &str, start: u32) -> Level {
    Level {
        start,
        format,
        text: text.to_owned(),
        alignment: Alignment::Left,
        indent: Some(Indent {
            left: Some(Length(3600)),
            right: None,
            first_line: Some(FirstLine::Hanging { by: Length(1800) }),
        }),
        run: None,
    }
}

fn round_trip(def: &AbstractNumbering) -> (AbstractNumbering, Log) {
    let mut log = Log::new("t");
    let back = read_list_style(
        &write_list_style("L1", def),
        def.id,
        &BTreeMap::new(),
        &mut log,
    );
    (back, log)
}

#[test]
fn spellable_lists_read_back_exactly() {
    let def = AbstractNumbering {
        id: AbstractNumId(4),
        levels: vec![
            level(NumberFormat::Decimal, "%1.", 1),
            level(NumberFormat::LowerLetter, "%1.%2)", 3),
            level(NumberFormat::UpperRoman, "(%3)", 1),
            Level {
                run: Some(RunProperties {
                    font: Some("Symbol".to_owned()),
                    color: Some(Color::rgb(1, 2, 3)),
                    ..RunProperties::none()
                }),
                alignment: Alignment::Right,
                ..level(NumberFormat::Bullet, "\u{2022}", 1)
            },
            level(NumberFormat::None, "%5.", 1),
        ],
    };
    let (back, log) = round_trip(&def);
    assert_eq!(back, def);
    assert!(log.report.is_lossless(), "{:?}", log.report);
}

#[test]
fn templates_odf_cannot_spell_are_recognised() {
    assert_eq!(spell("%1.%2.", 2), Some((String::new(), 2, ".".to_owned())));
    assert_eq!(
        spell("Art. %1", 1),
        Some(("Art. ".to_owned(), 1, String::new()))
    );
    assert_eq!(spell("%1.", 2), None, "a parent's number alone");
    assert_eq!(spell("%1-%2", 2), None, "a separator other than a dot");
    assert_eq!(spell("%2.%1", 2), None);
    assert_eq!(spell("\u{2022}", 1), None);
    let def = AbstractNumbering {
        id: AbstractNumId(1),
        levels: vec![
            level(NumberFormat::Decimal, "%1.", 0),
            level(NumberFormat::Decimal, "%1.", 1),
        ],
    };
    let (back, _) = round_trip(&def);
    assert_ne!(
        back, def,
        "start 0 and a parent-only label have no ODF slot"
    );
    assert_eq!(
        (back.levels[0].start, back.levels[1].text.as_str()),
        (1, "%2")
    );
}

#[test]
fn a_libreoffice_list_style_reads() {
    let xml = br#"<text:list-style style:name="L1">
      <text:list-level-style-bullet text:level="1" text:style-name="Bullet_20_Symbols" text:bullet-char="&#x2022;">
        <style:list-level-properties text:list-level-position-and-space-mode="label-alignment">
          <style:list-level-label-alignment text:label-followed-by="listtab" text:list-tab-stop-position="1.27cm" fo:text-indent="-0.635cm" fo:margin-left="1.27cm"/>
        </style:list-level-properties>
      </text:list-level-style-bullet>
      <text:list-level-style-number text:level="2" style:num-suffix="." style:num-format="1" text:display-levels="2"/>
    </text:list-style>"#;
    let node = cdoc_pkg::xml::parse("t", xml, cdoc_pkg::xml::Whitespace::Keep).expect("parses");
    let mut log = Log::new("t");
    let def = read_list_style(&node, AbstractNumId(1), &BTreeMap::new(), &mut log);
    assert_eq!(def.levels.len(), 2);
    assert_eq!(def.levels[0].text, "\u{2022}");
    assert_eq!(
        def.levels[0].indent,
        Some(Indent {
            left: Some(Length(3600)),
            right: None,
            first_line: Some(FirstLine::Hanging { by: Length(1800) })
        })
    );
    assert_eq!(def.levels[1].text, "%1.%2.");
    assert_eq!(log.report.count_of("a list label's character style"), 1);
}

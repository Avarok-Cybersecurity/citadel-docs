use super::*;

fn setup(top: i32, header: i32, gutter: i32) -> PageSetup {
    let mut page = PageSetup::a4_normal();
    page.margins.top = Length(top);
    page.margins.header = Length(header);
    page.margins.gutter = Length(gutter);
    page.columns = Columns {
        count: 2,
        gap: Length(1440),
        separator: true,
    };
    page.page_numbering = PageNumbering {
        format: NumberFormat::LowerRoman,
        start: Some(3),
    };
    page.size = page.size.rotated();
    page
}

fn round_trip(page: &PageSetup, header: bool, footer: bool) -> (PageSetup, Log) {
    let mut log = Log::new("t");
    let back = read_layout(
        &write_layout("pm1", page, header, footer),
        header,
        footer,
        page.page_numbering.start,
        &mut log,
    );
    (back, log)
}

#[test]
fn with_header_and_footer_every_margin_reads_back() {
    let page = setup(7200, 720, 0);
    let (back, log) = round_trip(&page, true, true);
    assert_eq!(back, page);
    assert!(log.report.is_lossless(), "{:?}", log.report);
}

#[test]
fn without_a_header_the_body_margin_is_exact_and_the_header_distance_derived() {
    let mut page = setup(7200, 3600, 0);
    page.margins.footer = Length(page.margins.bottom.0 / 2);
    let (back, _) = round_trip(&page, false, false);
    assert_eq!(back, page, "half the margin, as the module documents");
    let mut other = page.clone();
    other.margins.header = Length(100);
    assert_ne!(round_trip(&other, false, false).0, other);
}

#[test]
fn a_header_lower_than_the_body_margin_and_a_gutter_have_no_slot() {
    let (back, _) = round_trip(&setup(500, 720, 0), true, true);
    assert_eq!(back.margins.top, Length(720));
    let (back, _) = round_trip(&setup(7200, 720, 360), true, true);
    assert_eq!(back.margins.gutter, Length(0));
}

#[test]
fn libreoffice_layouts_read() {
    let xml = br#"<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="21.001cm" fo:page-height="29.7cm" style:num-format="1" style:print-orientation="portrait" fo:margin-top="2cm" fo:margin-bottom="2cm" fo:margin-left="2cm" fo:margin-right="2cm" style:writing-mode="lr-tb" style:footnote-max-height="0cm"><style:footnote-sep style:width="0.018cm"/></style:page-layout-properties><style:header-style><style:header-footer-properties fo:min-height="0cm" fo:margin-bottom="0.499cm"/></style:header-style><style:footer-style/></style:page-layout>"#;
    let node = cdoc_pkg::xml::parse(
        "t",
        xml,
        cdoc_pkg::xml::Whitespace::Keep,
        &cdoc_pkg::ReadLimits::generous(),
    )
    .expect("parses");
    let mut log = Log::new("t");
    let page = read_layout(&node, true, false, None, &mut log);
    assert_eq!(page.size.width, Length(59_530), "21.001cm");
    assert_eq!(
        (page.margins.header, page.margins.top),
        (Length(5669), Length(5669 + 1414))
    );
    assert_eq!(page.margins.footer, Length(5669 / 2));
    assert_eq!(
        log.report
            .count_of("style:page-layout-properties@style:writing-mode"),
        1
    );
    assert_eq!(
        log.report
            .count_of("style:page-layout-properties/style:footnote-sep"),
        1
    );
}

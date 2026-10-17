//! Reading hand-written ODF, one mapping per test: lists, tables, images, tracked changes,
//! annotations, and sections from master pages.

mod common;

use cdoc_model::{Block, Document, InlineContent, Length, Report, RevisionKind};

fn read(autos: &str, body: &str, styles: &str) -> (Document, Report) {
    common::read(&common::odt(autos, body, styles))
}

fn lost(report: &Report, what: &str) -> bool {
    report.has(|_| true, |w| w == what)
}

const LIST: &str = "<text:list-style style:name=\"L1\"><text:list-level-style-number text:level=\"1\" style:num-format=\"1\" style:num-suffix=\".\"/>\
    <text:list-level-style-number text:level=\"2\" style:num-format=\"a\" style:num-suffix=\")\"/></text:list-style>";

#[test]
fn nested_lists_give_levels_and_continued_lists_share_an_instance() {
    let body = "<text:list xml:id=\"a\" text:style-name=\"L1\"><text:list-item><text:p>1</text:p>\
        <text:list><text:list-item><text:p>1a</text:p></text:list-item></text:list></text:list-item></text:list>\
        <text:p>between</text:p>\
        <text:list text:style-name=\"L1\" text:continue-list=\"a\"><text:list-item><text:p>2</text:p></text:list-item></text:list>\
        <text:list text:style-name=\"L1\"><text:list-item><text:p>new</text:p></text:list-item></text:list>";
    let (doc, _) = read(LIST, body, "");
    let refs: Vec<_> = doc
        .paragraphs()
        .map(|p| p.properties.numbering.map(|n| (n.num_id.0, n.level)))
        .collect();
    assert_eq!(
        refs,
        [Some((1, 0)), Some((1, 1)), None, Some((1, 0)), Some((2, 0))]
    );
    assert_eq!(doc.numbering.definitions[0].levels[1].text, "%2)");
}

#[test]
fn table_spans_cover_cells_and_header_rows_mark_headers() {
    let autos = "<style:style style:name=\"C1\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"1in\"/></style:style>";
    let body = "<table:table><table:table-column table:style-name=\"C1\" table:number-columns-repeated=\"2\"/>\
        <table:table-header-rows><table:table-row><table:table-cell><text:p>h1</text:p></table:table-cell>\
        <table:table-cell><text:p>h2</text:p></table:table-cell></table:table-row></table:table-header-rows>\
        <table:table-row><table:table-cell table:number-columns-spanned=\"2\" table:number-rows-spanned=\"2\"><text:p>m</text:p></table:table-cell>\
        <table:covered-table-cell/></table:table-row><table:table-row><table:covered-table-cell/><table:covered-table-cell/></table:table-row></table:table>";
    let (doc, _) = read(autos, body, "");
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("a table")
    };
    assert_eq!(t.column_widths, [Length(7200), Length(7200)]);
    assert_eq!(
        t.rows.iter().map(|r| r.cells.len()).collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert!(t.rows[0].cells.iter().all(|c| c.header));
    let merged = &t.rows[1].cells[0].properties;
    assert_eq!(
        (merged.col_span, merged.row_span, t.rows[1].cells[0].header),
        (2, 2, false)
    );
}

#[test]
fn an_image_goes_to_the_media_sink_with_its_size() {
    let body =
        "<text:p><draw:frame text:anchor-type=\"as-char\" svg:width=\"2in\" svg:height=\"1in\">\
        <draw:image xlink:href=\"Pictures/a.png\"/><svg:desc>alt</svg:desc></draw:frame></text:p>";
    let mut bytes = common::odt("", body, "");
    bytes = {
        let mut parts: Vec<(String, Vec<u8>)> = common::entries(&bytes)
            .into_iter()
            .map(|(n, _, d)| (n, d))
            .collect();
        parts.push(("Pictures/a.png".to_owned(), b"\x89PNG-bytes".to_vec()));
        let text: Vec<(String, String)> = parts
            .into_iter()
            .map(|(n, d)| (n, String::from_utf8_lossy(&d).into_owned()))
            .collect();
        common::package(
            &text
                .iter()
                .map(|(n, t)| (n.as_str(), t.as_str()))
                .collect::<Vec<_>>(),
        )
    };
    let (doc, report, media) = common::read_foreign(&bytes);
    let p = doc.paragraphs().next().expect("p");
    let InlineContent::Image(img) = &p.inlines[0].content else {
        panic!("an image")
    };
    assert_eq!(
        (img.width, img.height, img.alt.as_deref()),
        (Length(14400), Length(7200), Some("alt"))
    );
    assert_eq!(media.ids(), std::slice::from_ref(&img.content_id));
    assert!(!report.has(|p| p == "Pictures/a.png", |_| true));
}

#[test]
fn changes_read_as_insertion_and_deletion_marks() {
    let body = "<text:tracked-changes text:track-changes=\"true\">\
        <text:changed-region text:id=\"i\"><text:insertion><office:change-info><dc:creator>A</dc:creator><dc:date>2024-01-01T00:00:00</dc:date></office:change-info></text:insertion></text:changed-region>\
        <text:changed-region text:id=\"d\"><text:deletion><office:change-info><dc:creator>B</dc:creator><dc:date>2024-01-02T00:00:00</dc:date></office:change-info><text:p>gone</text:p></text:deletion></text:changed-region>\
        </text:tracked-changes><text:p>a<text:change-start text:change-id=\"i\"/>new<text:change-end text:change-id=\"i\"/><text:change text:change-id=\"d\"/>b</text:p>";
    let (doc, _) = read("", body, "");
    assert!(doc.settings.track_changes);
    let p = doc.paragraphs().next().expect("p");
    let marks: Vec<_> = p
        .inlines
        .iter()
        .map(|i| {
            i.marks
                .revision
                .as_ref()
                .map(|r| (r.kind, r.author.as_str()))
        })
        .collect();
    assert_eq!(
        marks,
        [
            None,
            Some((RevisionKind::Insertion, "A")),
            Some((RevisionKind::Deletion, "B")),
            None
        ]
    );
    assert!(matches!(&p.inlines[2].content, InlineContent::Text(t) if t.text == "gone"));
}

#[test]
fn an_annotation_with_an_end_marks_a_range_across_paragraphs() {
    let body = "<text:p>a<office:annotation office:name=\"c1\"><dc:creator>Ann</dc:creator><dc:date>2024-01-01</dc:date>\
        <text:p>one</text:p><text:p>two</text:p></office:annotation>b</text:p><text:p>c<office:annotation-end office:name=\"c1\"/>d</text:p>";
    let (doc, report) = read("", body, "");
    let c = &doc.comments[0];
    assert_eq!(
        (c.id.as_str(), c.author.as_str(), c.text.as_str()),
        ("c1", "Ann", "one\ntwo")
    );
    let marked: Vec<bool> = doc
        .inlines()
        .map(|i| !i.marks.comments.is_empty())
        .collect();
    assert_eq!(marked, [false, true, true, false]);
    assert!(!lost(
        &report,
        "a comment on a point kept without an anchor"
    ));
}

#[test]
fn a_point_annotation_keeps_its_comment_without_an_anchor() {
    let (doc, report) = read(
        "",
        "<text:p>a<office:annotation><text:p>x</text:p></office:annotation></text:p>",
        "",
    );
    assert_eq!(doc.comments.len(), 1);
    assert!(doc.inlines().all(|i| i.marks.comments.is_empty()));
    assert!(lost(&report, "a comment on a point kept without an anchor"));
}

#[test]
fn a_master_page_change_starts_a_section_with_its_headers_and_title_page() {
    let styles_autos = "<style:style style:name=\"P9\" style:family=\"paragraph\" style:master-page-name=\"First\">\
        <style:paragraph-properties style:page-number=\"5\"/></style:style>";
    let styles = "";
    let body = "<text:p>one</text:p><text:p text:style-name=\"P9\">two</text:p>";
    let mut bytes = common::odt(styles_autos, body, styles);
    let masters = "<style:master-page style:name=\"Standard\" style:page-layout-name=\"pm1\"/>\
        <style:master-page style:name=\"First\" style:page-layout-name=\"pm1\" style:next-style-name=\"Main\"><style:header><text:p>F</text:p></style:header></style:master-page>\
        <style:master-page style:name=\"Main\" style:page-layout-name=\"pm1\"><style:header><text:p>H</text:p></style:header>\
        <style:header-left><text:p>E</text:p></style:header-left></style:master-page>";
    let parts: Vec<(String, String)> = common::entries(&bytes)
        .into_iter()
        .map(|(n, _, d)| {
            let t = String::from_utf8(d).expect("utf-8");
            let t = if n == "styles.xml" {
                t.replace(
                    "<style:master-page style:name=\"Standard\" style:page-layout-name=\"pm1\"/>",
                    masters,
                )
            } else {
                t
            };
            (n, t)
        })
        .collect();
    bytes = common::package(
        &parts
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect::<Vec<_>>(),
    );
    let (doc, _) = common::read(&bytes);
    assert_eq!(doc.sections.len(), 2);
    let second = &doc.sections[1].properties;
    assert!(second.title_page);
    assert_eq!(second.page.page_numbering.start, Some(5));
    let header_text = |hf: &Option<cdoc_model::HeaderFooter>| {
        hf.as_ref().and_then(|h| {
            h.blocks[0]
                .paragraphs()
                .next()
                .and_then(|p| match &p.inlines[0].content {
                    InlineContent::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
        })
    };
    assert_eq!(header_text(&second.headers.first).as_deref(), Some("F"));
    assert_eq!(header_text(&second.headers.default).as_deref(), Some("H"));
    assert_eq!(header_text(&second.headers.even).as_deref(), Some("E"));
    assert!(doc.settings.even_and_odd_headers);
    assert_eq!(second.page.size.width, Length(59_530));
}

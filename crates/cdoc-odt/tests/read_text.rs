//! Reading hand-written ODF, one mapping per test: text and white space, headings, automatic
//! styles as direct formatting, hyperlinks, breaks, fields, footnotes, bookmarks and soft page
//! breaks (reported, not kept).

mod common;

use cdoc_model::{BreakKind, Document, FieldKind, InlineContent, Length, Report, StyleId};

fn read(autos: &str, body: &str) -> (Document, Report) {
    common::read(&common::odt(autos, body, ""))
}

fn texts(doc: &Document) -> Vec<String> {
    doc.inlines()
        .filter_map(|i| match &i.content {
            InlineContent::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn lost(report: &Report, what: &str) -> bool {
    report.has(|_| true, |w| w == what)
}

#[test]
fn white_space_collapses_and_text_s_keeps_spaces() {
    let (doc, _) = read(
        "",
        "<text:p>  a \n  b<text:s text:c=\"3\"/>c<text:tab/>d</text:p>",
    );
    assert_eq!(texts(&doc), ["a b   c", "d"]);
    let p = doc.paragraphs().next().expect("a paragraph");
    assert!(matches!(p.inlines[1].content, InlineContent::Tab));
}

#[test]
fn a_heading_takes_its_outline_level() {
    let (doc, _) = read("", "<text:h text:outline-level=\"3\">T</text:h>");
    assert_eq!(
        doc.paragraphs().next().expect("p").properties.outline_level,
        Some(2)
    );
}

#[test]
fn an_automatic_style_becomes_direct_formatting_on_its_named_style() {
    let autos = "<style:style style:name=\"P1\" style:family=\"paragraph\" style:parent-style-name=\"Body\">\
        <style:paragraph-properties fo:text-align=\"center\" fo:margin-left=\"1in\"/></style:style>\
        <style:style style:name=\"T1\" style:family=\"text\"><style:text-properties fo:font-weight=\"bold\" fo:font-size=\"14pt\"/></style:style>";
    let styles = "<style:style style:name=\"Body\" style:family=\"paragraph\"/>";
    let (doc, _) = common::read(&common::odt(
        autos,
        "<text:p text:style-name=\"P1\"><text:span text:style-name=\"T1\">x</text:span></text:p>",
        styles,
    ));
    let p = doc.paragraphs().next().expect("p");
    assert_eq!(p.properties.style, Some(StyleId::new("Body")));
    assert_eq!(p.properties.alignment, Some(cdoc_model::Alignment::Center));
    assert_eq!(
        p.properties.indent.as_ref().and_then(|i| i.left),
        Some(Length(7200))
    );
    let InlineContent::Text(run) = &p.inlines[0].content else {
        panic!("text")
    };
    assert_eq!(run.properties.bold, Some(true));
    assert_eq!(run.properties.size, Some(cdoc_model::FontSize(28)));
}

#[test]
fn nested_spans_overlay_their_properties() {
    let autos = "<style:style style:name=\"T1\" style:family=\"text\"><style:text-properties fo:font-weight=\"bold\"/></style:style>\
        <style:style style:name=\"T2\" style:family=\"text\"><style:text-properties fo:font-style=\"italic\"/></style:style>";
    let (doc, _) = read(autos, "<text:p><text:span text:style-name=\"T1\">a<text:span text:style-name=\"T2\">b</text:span></text:span></text:p>");
    let p = doc.paragraphs().next().expect("p");
    let InlineContent::Text(b) = &p.inlines[1].content else {
        panic!("text")
    };
    assert_eq!(
        (b.properties.bold, b.properties.italic),
        (Some(true), Some(true))
    );
}

#[test]
fn a_hyperlink_marks_its_content() {
    let (doc, _) = read(
        "",
        "<text:p><text:a xlink:href=\"https://e.x/\" office:title=\"t\">go</text:a> on</text:p>",
    );
    let p = doc.paragraphs().next().expect("p");
    let link = p.inlines[0].marks.link.as_ref().expect("a link");
    assert_eq!(
        (link.href.as_str(), link.title.as_deref()),
        ("https://e.x/", Some("t"))
    );
    assert!(p.inlines[1].marks.link.is_none());
}

#[test]
fn line_breaks_and_fields_are_inline_content() {
    let (doc, _) = read("", "<text:p>a<text:line-break/><text:page-number>4</text:page-number><text:page-count>9</text:page-count></text:p>");
    let p = doc.paragraphs().next().expect("p");
    assert!(matches!(
        p.inlines[1].content,
        InlineContent::Break {
            kind: BreakKind::Line
        }
    ));
    let InlineContent::Field(f) = &p.inlines[2].content else {
        panic!("a field")
    };
    assert_eq!((&f.field, f.cached.as_str()), (&FieldKind::PageNumber, "4"));
    assert!(
        matches!(&p.inlines[3].content, InlineContent::Field(f) if f.field == FieldKind::PageCount)
    );
}

#[test]
fn a_date_field_takes_its_data_style_as_a_picture() {
    let autos = "<number:date-style xmlns:number=\"urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0\" style:name=\"N1\">\
        <number:day number:style=\"long\"/><number:text>.</number:text><number:month number:style=\"long\"/>\
        <number:text>.</number:text><number:year number:style=\"long\"/></number:date-style>";
    let (doc, _) = read(
        autos,
        "<text:p><text:date style:data-style-name=\"N1\">01.02.2024</text:date></text:p>",
    );
    let p = doc.paragraphs().next().expect("p");
    let InlineContent::Field(f) = &p.inlines[0].content else {
        panic!("a field")
    };
    assert_eq!(
        f.field,
        FieldKind::Date {
            format: "dd.MM.yyyy".to_owned()
        }
    );
}

#[test]
fn a_note_becomes_a_footnote_and_an_endnote_is_reported() {
    let body = "<text:p>a<text:note text:id=\"ftn7\" text:note-class=\"footnote\"><text:note-citation>1</text:note-citation>\
        <text:note-body><text:p>n</text:p></text:note-body></text:note>\
        <text:note text:id=\"edn1\" text:note-class=\"endnote\"><text:note-body><text:p>e</text:p></text:note-body></text:note></text:p>";
    let (doc, report) = read("", body);
    assert_eq!(
        doc.footnotes.iter().map(|f| f.id.0).collect::<Vec<_>>(),
        [7, 1]
    );
    let p = doc.sections[0].blocks[0].paragraphs().next().expect("p");
    assert!(matches!(p.inlines[1].content, InlineContent::FootnoteRef { id } if id.0 == 7));
    assert!(lost(&report, "an endnote read as a footnote"));
}

#[test]
fn bookmarks_and_soft_page_breaks_are_ignored_with_a_report() {
    let (doc, report) = read("", "<text:p>a<text:bookmark text:name=\"b\"/>b</text:p><text:soft-page-break/><text:p>c</text:p>");
    assert_eq!(texts(&doc), ["ab", "c"]);
    assert!(lost(&report, "a bookmark (ignored)"));
    assert!(lost(&report, "a soft page break (ignored)"));
}

#[test]
fn tab_stops_are_reported() {
    let autos = "<style:style style:name=\"P1\" style:family=\"paragraph\"><style:paragraph-properties>\
        <style:tab-stops><style:tab-stop style:position=\"1in\"/></style:tab-stops></style:paragraph-properties></style:style>";
    let (_, report) = read(autos, "<text:p text:style-name=\"P1\">a</text:p>");
    assert!(lost(&report, "style:paragraph-properties/style:tab-stops"));
}

#[test]
fn an_unknown_inline_element_is_read_as_its_text_and_reported() {
    let (doc, report) = read(
        "",
        "<text:p><text:sender-firstname>Ann</text:sender-firstname></text:p>",
    );
    assert_eq!(texts(&doc), ["Ann"]);
    assert!(lost(&report, "text:sender-firstname read as its text"));
}

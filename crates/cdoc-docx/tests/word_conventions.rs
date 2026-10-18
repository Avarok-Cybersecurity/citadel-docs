//! Constructs Word writes that the model has no direct slot for, hand-authored as the XML
//! Word emits. Each is either mapped (hyperlink fields, tracked changes, vertical merges) or
//! reported; none is dropped silently.

mod common;

use cdoc_model::{Block, InlineContent, RevisionKind};

fn read_body(body: &str) -> (cdoc_model::Document, cdoc_docx::Report) {
    let (doc, report, _) = common::read_foreign(&common::word_package(body, "", &[]));
    (doc, report)
}

#[test]
fn hyperlink_fields_become_links() {
    let (doc, report) = read_body(
        "<w:p><w:fldSimple w:instr=\" HYPERLINK &quot;https://a.test/x&quot; \\o &quot;tip&quot; \">\
         <w:r><w:t>simple</w:t></w:r></w:fldSimple>\
         <w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> HYPERLINK \\l \"top\" </w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>complex</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
    );
    let links: Vec<_> = doc.inlines().map(|i| i.marks.link.clone()).collect();
    assert_eq!(
        links[0]
            .as_ref()
            .map(|l| (l.href.as_str(), l.title.as_deref())),
        Some(("https://a.test/x", Some("tip")))
    );
    assert_eq!(links[1].as_ref().map(|l| l.href.as_str()), Some("#top"));
    assert!(!report.has(|_| true, |w| w.contains("HYPERLINK")));
}

#[test]
fn unknown_elements_are_reported_with_where_they_were() {
    let (doc, report) = read_body(
        "<w:p><w:pPr><w:tabs><w:tab w:val=\"left\" w:pos=\"720\"/></w:tabs></w:pPr>\
         <w:smartTag><w:r><w:t>tagged</w:t></w:r></w:smartTag>\
         <w:r><w:rPr><w:spacing w:val=\"20\"/></w:rPr><w:t>kept</w:t></w:r><w:bookmarkStart w:id=\"0\" w:name=\"b\"/><w:bookmarkEnd w:id=\"0\"/></w:p>\
         <w:p><w:r><w:endnoteReference w:id=\"1\"/></w:r></w:p>",
    );
    let wanted = [
        "w:pPr/w:tabs",
        "w:p/w:smartTag",
        "w:rPr/w:spacing",
        "w:bookmarkStart",
        "w:bookmarkEnd",
        "w:r/w:endnoteReference",
    ];
    for what in wanted {
        assert_eq!(
            report.count_of(what),
            1,
            "{what} should be reported once: {report:?}"
        );
    }
    assert!(doc
        .inlines()
        .any(|i| matches!(&i.content, InlineContent::Text(t) if t.text == "kept")));
}

#[test]
fn tracked_changes_keep_author_date_and_kind_and_number_their_ids() {
    let (doc, _) = read_body(
        "<w:p><w:ins w:id=\"7\" w:author=\"Ann\" w:date=\"2026-01-02T03:04:05Z\"><w:r><w:t>new</w:t></w:r></w:ins>\
         <w:del w:id=\"8\" w:author=\"Bo\" w:date=\"2026-01-02T03:05:00Z\"><w:r><w:delText>old</w:delText></w:r></w:del></w:p>",
    );
    let revs: Vec<_> = doc
        .inlines()
        .map(|i| i.marks.revision.clone().expect("revision"))
        .collect();
    assert_eq!(
        (revs[0].kind, revs[0].id.as_str(), revs[0].author.as_str()),
        (RevisionKind::Insertion, "7", "Ann")
    );
    assert_eq!(
        (revs[1].kind, revs[1].date.as_str()),
        (RevisionKind::Deletion, "2026-01-02T03:05:00Z")
    );
    let InlineContent::Text(t) = &doc.inlines().nth(1).expect("deleted run").content else {
        panic!()
    };
    assert_eq!(
        t.text, "old",
        "deleted text is kept, as the model's deletion mark expects"
    );
}

#[test]
fn dangling_references_are_dropped_and_reported() {
    let (doc, report) = read_body(
        "<w:p><w:pPr><w:pStyle w:val=\"Nope\"/><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"9\"/></w:numPr></w:pPr>\
         <w:r><w:rPr><w:rStyle w:val=\"Ghost\"/></w:rPr><w:t>x</w:t></w:r>\
         <w:commentRangeStart w:id=\"4\"/><w:r><w:t>y</w:t></w:r><w:commentRangeEnd w:id=\"4\"/>\
         <w:r><w:footnoteReference w:id=\"3\"/></w:r></w:p>",
    );
    assert_eq!(doc.validate(), Ok(()));
    for what in [
        "a paragraph style that is not defined; dropped",
        "a character style that is not defined; dropped",
        "a paragraph in a list that is not defined; list dropped",
        "a comment anchor with no comment; dropped",
        "a footnote reference with no footnote; dropped",
    ] {
        assert!(report.count_of(what) >= 1, "{what}: {report:?}");
    }
}

#[test]
fn block_level_content_controls_are_unwrapped_not_lost() {
    let (doc, report) = read_body(
        "<w:sdt><w:sdtPr><w:docPartObj><w:docPartGallery w:val=\"Table of Contents\"/></w:docPartObj></w:sdtPr>\
         <w:sdtContent><w:p><w:r><w:t>Contents</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:r><w:t>After</w:t></w:r></w:p>",
    );
    assert_eq!(doc.paragraphs().count(), 2);
    assert_eq!(report.count_of("block-level content control unwrapped"), 1);
}

#[test]
fn vertical_merge_from_word_becomes_a_row_span() {
    let cell = |extra: &str| {
        format!("<w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/>{extra}</w:tcPr><w:p/></w:tc>")
    };
    let table = format!(
        "<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"2000\"/></w:tblGrid>\
         <w:tr>{}{}</w:tr><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl><w:p/>",
        cell("<w:vMerge w:val=\"restart\"/>"), cell(""),
        cell("<w:vMerge/>"), cell(""),
        cell(""), cell(""),
    );
    let (doc, _) = read_body(&table);
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("a table first")
    };
    assert_eq!(t.rows[0].cells[0].properties.row_span, 2);
    assert_eq!(
        t.rows[1].cells.len(),
        1,
        "the continuation cell is folded into the span"
    );
    assert_eq!(t.rows[2].cells.len(), 2);
}

#[test]
fn floating_and_unreadable_drawings_are_reported() {
    let drawing = "<w:p><w:r><w:drawing><wp:anchor><wp:extent cx=\"127000\" cy=\"127000\"/>\
        <a:graphic><a:graphicData uri=\"x\"><pic:pic><pic:blipFill><a:blip r:embed=\"rId9\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>\
        <w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"1\" cy=\"1\"/><a:graphic><a:graphicData uri=\"chart\"/></a:graphic></wp:inline></w:drawing></w:r></w:p>";
    let png = "<Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/p.png\"/>";
    let bytes = common::word_package(drawing, png, &[]);
    let (_, report, media) = common::read_foreign(&bytes);
    assert_eq!(report.count_of("floating image placed inline"), 1);
    assert_eq!(
        report.count_of("drawing that is not a picture (shape, chart or diagram)"),
        1
    );
    assert_eq!(media.ids().len(), 0);
}

#[test]
fn comments_with_replies_and_resolution_read_back() {
    let rels = "<Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments\" Target=\"comments.xml\"/>\
        <Relationship Id=\"rId6\" Type=\"http://schemas.microsoft.com/office/2011/relationships/commentsExtended\" Target=\"commentsExtended.xml\"/>";
    let comments = "<w:comments xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\">\
        <w:comment w:id=\"0\" w:author=\"A\" w:date=\"2026-01-01T00:00:00Z\"><w:p w14:paraId=\"0000000A\"><w:r><w:t>root</w:t></w:r></w:p></w:comment>\
        <w:comment w:id=\"1\" w:author=\"B\" w:date=\"2026-01-01T01:00:00Z\"><w:p w14:paraId=\"0000000B\"><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p></w:comment></w:comments>";
    let ext = "<w15:commentsEx xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\">\
        <w15:commentEx w15:paraId=\"0000000A\" w15:done=\"1\"/><w15:commentEx w15:paraId=\"0000000B\" w15:paraIdParent=\"0000000A\" w15:done=\"0\"/></w15:commentsEx>";
    let bytes = common::word_package(
        "<w:p><w:commentRangeStart w:id=\"0\"/><w:r><w:t>t</w:t></w:r><w:commentRangeEnd w:id=\"0\"/></w:p>",
        rels,
        &[("word/comments.xml", comments), ("word/commentsExtended.xml", ext)],
    );
    let (doc, _, _) = common::read_foreign(&bytes);
    assert_eq!(doc.validate(), Ok(()));
    assert!(doc.comments[0].resolved && doc.comments[0].parent.is_none());
    assert_eq!(doc.comments[1].parent.as_ref(), Some(&doc.comments[0].id));
    assert_eq!(doc.comments[1].text, "one\ntwo");
}

#[test]
fn headers_carry_over_to_later_sections_without_their_own() {
    let rels = "<Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header1.xml\"/>";
    let header = "<w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:p><w:r><w:t>Top</w:t></w:r></w:p></w:hdr>";
    let body = "<w:p><w:pPr><w:sectPr><w:headerReference w:type=\"default\" r:id=\"rId7\"/></w:sectPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p>\
                <w:p><w:r><w:t>two</w:t></w:r></w:p><w:sectPr/>";
    let bytes = common::word_package(body, rels, &[("word/header1.xml", header)]);
    let (doc, _, _) = common::read_foreign(&bytes);
    assert_eq!(doc.sections.len(), 2);
    assert!(
        doc.sections[1].properties.headers.default.is_some(),
        "Word continues the header"
    );
}

#[test]
fn repair_reaches_table_cells_footnotes_and_headers() {
    let rels = "<Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header1.xml\"/>\
        <Relationship Id=\"rId8\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes\" Target=\"footnotes.xml\"/>";
    let w = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"";
    let ghost = "<w:pPr><w:pStyle w:val=\"Ghost\"/></w:pPr>";
    let header = format!("<w:hdr {w}><w:p>{ghost}<w:r><w:t>h</w:t></w:r></w:p></w:hdr>");
    let notes = format!("<w:footnotes {w}><w:footnote w:id=\"1\"><w:p>{ghost}<w:r><w:t>n</w:t></w:r></w:p></w:footnote></w:footnotes>");
    let body = format!(
        "<w:p><w:r><w:footnoteReference w:id=\"1\"/></w:r></w:p>\
         <w:tbl><w:tblGrid><w:gridCol w:w=\"2000\"/></w:tblGrid><w:tr><w:tc><w:p>{ghost}<w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>\
         <w:sectPr><w:headerReference w:type=\"default\" r:id=\"rId7\"/></w:sectPr>"
    );
    let bytes = common::word_package(
        &body,
        rels,
        &[
            ("word/header1.xml", &header),
            ("word/footnotes.xml", &notes),
        ],
    );
    let (doc, report, _) = common::read_foreign(&bytes);
    assert_eq!(doc.validate(), Ok(()));
    assert_eq!(
        report.count_of("a paragraph style that is not defined; dropped"),
        3
    );
    assert!(doc.paragraphs().all(|p| p.properties.style.is_none()));
}

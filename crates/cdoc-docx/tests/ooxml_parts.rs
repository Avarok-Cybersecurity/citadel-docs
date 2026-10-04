//! What the writer emits, checked against the OOXML vocabulary itself: a round trip alone
//! would pass for a private format, so these look for the real element and attribute names.

mod common;

use cdoc_model::test_support::fixtures::kitchen_sink;

fn sink() -> (Vec<u8>, String) {
    let (bytes, _) = common::write(&kitchen_sink());
    let doc = common::part_text(&bytes, "word/document.xml");
    (bytes, doc)
}

#[test]
fn package_has_the_parts_word_expects() {
    let (bytes, _) = sink();
    let names = common::part_names(&bytes);
    for expected in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/numbering.xml",
        "word/settings.xml",
        "word/footnotes.xml",
        "word/comments.xml",
        "word/commentsExtended.xml",
        "word/header1.xml",
        "word/footer1.xml",
        "word/media/image1.png",
        "docProps/core.xml",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} missing from {names:?}"
        );
    }
    assert_eq!(
        names[0], "[Content_Types].xml",
        "content types must come first"
    );
}

#[test]
fn body_uses_wordprocessingml_vocabulary() {
    let (_, doc) = sink();
    for needle in [
        "<w:p>",
        "<w:pStyle w:val=\"Heading1\"/>",
        "<w:numPr>",
        "<w:ilvl w:val=\"1\"/>",
        "<w:hyperlink ",
        "<w:ins ",
        "<w:del ",
        "<w:delText ",
        "<w:commentRangeStart w:id=\"0\"/>",
        "<w:commentRangeEnd w:id=\"1\"/>",
        "<w:commentReference ",
        "<w:footnoteReference w:id=\"1\"/>",
        "<w:fldChar w:fldCharType=\"begin\"/>",
        "<w:instrText xml:space=\"preserve\"> DATE ",
        "<w:tab/>",
        "<w:br w:type=\"page\"/>",
        "<w:br/>",
        "<w:drawing>",
        "<wp:extent cx=\"1828800\" cy=\"914400\"/>",
        "<w:sdt>",
        "<w:dropDownList ",
        "<w:tbl>",
        "<w:tblGrid>",
        "<w:gridSpan w:val=\"2\"/>",
        "<w:trHeight ",
        "<w:cantSplit/>",
        "<w:vAlign w:val=\"center\"/>",
        "<w:sectPr>",
        "<w:titlePg/>",
        "<w:type w:val=\"oddPage\"/>",
        "<w:pgSz w:w=\"16838\" w:h=\"11906\" w:orient=\"landscape\"/>",
        "<w:cols w:num=\"2\" ",
        "<w:headerReference w:type=\"first\" ",
        "<w:headerReference w:type=\"even\" ",
        "<w:footerReference w:type=\"default\" ",
    ] {
        assert!(doc.contains(needle), "{needle} not in document.xml");
    }
    assert!(
        doc.contains("<w:sz w:val=\"28\"/>"),
        "14pt is 28 half-points"
    );
    assert!(doc.contains("<w:u w:val=\"double\" w:color=\"FF0000\"/>"));
}

#[test]
fn companion_parts_use_wordprocessingml_vocabulary() {
    let (bytes, _) = sink();
    let settings = common::part_text(&bytes, "word/settings.xml");
    assert!(
        settings.contains("<w:evenAndOddHeaders/>")
            && settings.contains("<w:defaultTabStop w:val=\"720\"/>")
    );
    let numbering = common::part_text(&bytes, "word/numbering.xml");
    for needle in [
        "<w:abstractNum w:abstractNumId=\"1\">",
        "<w:lvl w:ilvl=\"1\">",
        "<w:lvlText w:val=\"%2.\"/>",
        "<w:num w:numId=\"1\">",
        "<w:lvlOverride w:ilvl=\"1\"><w:startOverride w:val=\"3\"/>",
    ] {
        assert!(numbering.contains(needle), "{needle} not in numbering.xml");
    }
    let styles = common::part_text(&bytes, "word/styles.xml");
    assert!(styles.contains("<w:docDefaults>") && styles.contains("w:styleId=\"Heading1\""));
    let notes = common::part_text(&bytes, "word/footnotes.xml");
    assert!(notes.contains("w:type=\"separator\"") && notes.contains("<w:footnoteRef/>"));
    let comments = common::part_text(&bytes, "word/commentsExtended.xml");
    assert!(comments.contains("w15:paraIdParent=") && comments.contains("w15:done=\"0\""));
    let core = common::part_text(&bytes, "docProps/core.xml");
    assert!(core.contains("<dc:title>Kitchen sink</dc:title>"));
}

#[test]
fn vertical_merges_use_vmerge() {
    use cdoc_model::{Block, Document};
    let mut doc = Document::blank();
    let Block::Table(mut table) = cdoc_model::test_support::fixtures::kitchen_sink().sections[0]
        .blocks
        .iter()
        .find(|b| matches!(b, Block::Table(_)))
        .cloned()
        .expect("table")
    else {
        unreachable!()
    };
    table.rows[0].cells[0].properties.row_span = 2;
    table.rows[1].cells.remove(0);
    doc.sections[0].blocks = vec![Block::Table(table)];
    let (bytes, _) = common::write(&doc);
    let xml = common::part_text(&bytes, "word/document.xml");
    assert!(xml.contains("<w:vMerge w:val=\"restart\"/>"));
    assert!(xml.contains("<w:vMerge/>"));
}

//! Reading Markdown: each construct maps as documented, or is reported.

mod common;

use cdoc_md::{document_to_md, md_to_document};
use cdoc_model::Alignment::{Center, Left, Right};
use cdoc_model::{Block, Document, InlineContent};

fn read(md: &str) -> (Document, cdoc_model::Report) {
    md_to_document(md, &common::options())
}

fn styles(doc: &Document) -> Vec<String> {
    doc.paragraphs()
        .map(|p| {
            p.properties
                .style
                .as_ref()
                .map_or(String::new(), |s| s.as_str().to_owned())
        })
        .collect()
}

#[test]
fn headings_quotes_and_rules_map_to_styles_and_borders() {
    let (doc, report) = read("# One\n\nSetext\n======\n\n###### Six\n\n> quoted\n\n---\n\nplain\n");
    assert_eq!(
        styles(&doc),
        ["Heading1", "Heading1", "Heading6", "Quote", "", "Normal"]
    );
    assert!(doc
        .paragraphs()
        .nth(4)
        .expect("rule")
        .properties
        .borders
        .is_some());
    assert!(report.is_lossless(), "{report:?}");
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn lists_become_flat_numbered_paragraphs_with_their_own_counters() {
    let (doc, _) = read("- a\n  - b\n- c\n\n3. x\n4. y\n\nbreak\n\n1. p\n");
    let refs: Vec<_> = doc
        .paragraphs()
        .map(|p| p.properties.numbering.map(|n| (n.num_id.0, n.level)))
        .collect();
    assert_eq!(refs[0], Some((1, 0)));
    assert_eq!(refs[1], Some((1, 1)));
    assert_eq!(refs[2], Some((1, 0)));
    assert_ne!(
        refs[3].map(|r| r.0),
        refs[6].map(|r| r.0),
        "each ordered list is its own instance"
    );
    let mut counters = cdoc_layout::labels::Counters::new(&doc.numbering);
    let labels: Vec<String> = doc
        .paragraphs()
        .filter_map(|p| p.properties.numbering)
        .filter_map(|r| counters.next(r))
        .map(|l| l.text)
        .collect();
    assert_eq!(
        labels,
        ["\u{2022}", "\u{25E6}", "\u{2022}", "3.", "4.", "1."]
    );
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn tables_keep_header_alignment_and_split_the_text_width() {
    let (doc, _) = read("| A | B | C |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n");
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("table")
    };
    assert!(t.rows[0].cells.iter().all(|c| c.header) && t.rows[1].cells.iter().all(|c| !c.header));
    let aligns: Vec<_> = t.rows[1]
        .cells
        .iter()
        .map(|c| match &c.blocks[0] {
            Block::Paragraph(p) => p.properties.alignment,
            Block::Table(_) => None,
        })
        .collect();
    assert_eq!(aligns, [Some(Left), Some(Center), Some(Right)]);
    let total: i32 = t.column_widths.iter().map(|w| w.0).sum();
    assert_eq!(total, common::options().text_width.0);
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn images_by_content_id_keep_the_id_and_urls_are_reported() {
    let (doc, report) = read("![alt text](cid:abc-1) and ![remote](https://x.test/a.png)\n");
    let image = doc
        .inlines()
        .find_map(|i| match &i.content {
            InlineContent::Image(img) => Some(img),
            _ => None,
        })
        .expect("image");
    assert_eq!(image.content_id.as_str(), "abc-1");
    assert_eq!(image.alt.as_deref(), Some("alt text"));
    assert_eq!(
        (image.width, image.height),
        (
            common::options().image_width,
            common::options().image_height
        )
    );
    assert_eq!(
        report.count_of("image with a URL, not a content id (its alt text is kept)"),
        1
    );
}

#[test]
fn footnotes_keep_their_numbers_and_names_get_ids() {
    let (doc, _) = read("a[^7] b[^note]\n\n[^7]: seven\n[^note]: named\n");
    let ids: Vec<u32> = doc.footnotes.iter().map(|f| f.id.0).collect();
    assert_eq!(ids, [7, 10_000]);
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn constructs_without_a_model_slot_are_reported_not_dropped_silently() {
    let (doc, report) = read("```rust\nlet x = 1;\nlet y = 2;\n```\n\n<div>block</div>\n\ntext <span>inline</span> <br> more\n");
    assert_eq!(
        report.count_of("code block (read as monospace paragraph)"),
        1
    );
    assert_eq!(report.count_of("HTML block (dropped)"), 1);
    assert_eq!(report.count_of("inline HTML (dropped)"), 2);
    let code = doc.paragraphs().next().expect("code paragraph");
    assert_eq!(code.inlines.len(), 3, "line, break, line");
    assert!(doc
        .paragraphs()
        .last()
        .expect("p")
        .inlines
        .iter()
        .any(|i| matches!(i.content, InlineContent::Break { .. })));
}

#[test]
fn an_empty_input_is_a_valid_blank_document() {
    let (doc, report) = read("");
    assert_eq!(doc.validate(), Ok(()));
    assert!(report.is_lossless());
}

#[test]
fn this_repositorys_own_markdown_reads_validly_and_rewrites_stably() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in ["README.md", "ARCHITECTURE.md", "docs/tiptap-schema.md"] {
        let text = std::fs::read_to_string(root.join(file)).expect("repo markdown present");
        let (doc, _) = read(&text);
        assert_eq!(doc.validate(), Ok(()), "{file}");
        assert!(doc.word_count() > 50, "{file} has content");
        let (written, _) = document_to_md(&doc);
        let (again, _) = read(&written);
        let (rewritten, _) = document_to_md(&again);
        let first_difference = written.lines().zip(rewritten.lines()).find(|(a, b)| a != b);
        assert_eq!(
            first_difference, None,
            "{file}: a second pass changes nothing"
        );
        assert_eq!(written.lines().count(), rewritten.lines().count(), "{file}");
    }
}

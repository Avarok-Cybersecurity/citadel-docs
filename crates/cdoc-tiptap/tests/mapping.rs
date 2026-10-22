//! Each `StarterKit` construct lands where `lib.rs` says, with the loss it says. Lists are in
//! `mapping_lists.rs`.

mod common;

use cdoc_model::builtin::{rule_paragraph, CODE_FONT};
use cdoc_model::{BreakKind, InlineContent, Severity};
use cdoc_tiptap::tiptap_to_document;
use common::{losses, read, style, t, text};
use serde_json::{json, Value};

#[test]
fn headings_are_the_heading_styles_and_other_levels_the_nearest() {
    let content: Vec<Value> = [1, 2, 3, 4, 5, 6, 0, 9]
        .iter()
        .map(|l| json!({"type": "heading", "attrs": {"level": l}, "content": [t("h")]}))
        .collect();
    let (ps, _, report) = read(&json!({"type": "doc", "content": content}));
    let styles: Vec<&str> = ps.iter().map(style).collect();
    assert_eq!(
        styles,
        [
            "Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6", "Heading1",
            "Heading6"
        ]
    );
    let approximated = "a heading level outside 1 to 6 read as the nearest";
    assert_eq!(report.count_of(approximated), 2);
    assert_eq!(
        losses(&report),
        [(approximated.to_owned(), Severity::Minor)]
    );
}

#[test]
fn a_plain_document_loses_nothing() {
    let doc = json!({"type": "doc", "content": [
        {"type": "paragraph", "content": [t("a"), {"type": "hardBreak"}, {"type": "text", "text": "b", "marks": [{"type": "bold"}, {"type": "italic"}, {"type": "strike"}]}]},
        {"type": "heading", "attrs": {"level": 2}, "content": [t("h")]},
        {"type": "horizontalRule"},
        {"type": "bulletList", "content": [{"type": "listItem", "content": [{"type": "paragraph", "content": [t("x")]}]}]},
    ]});
    let (ps, _, report) = read(&doc);
    assert!(report.is_lossless(), "{:?}", report.losses);
    assert_eq!(text(&ps[0]), "a\nb");
    let InlineContent::Text(b) = &ps[0].inlines[2].content else {
        panic!("text")
    };
    assert_eq!(
        (b.properties.bold, b.properties.italic, b.properties.strike),
        (Some(true), Some(true), Some(true))
    );
    assert_eq!(ps[2], rule_paragraph());
}

#[test]
fn quotes_code_and_their_neighbours_are_approximated_and_reported() {
    let doc = json!({"type": "doc", "content": [
        {"type": "blockquote", "content": [
            {"type": "paragraph", "content": [t("q")]},
            {"type": "blockquote", "content": [{"type": "paragraph", "content": [t("qq")]}]},
            {"type": "heading", "attrs": {"level": 1}, "content": [t("qh")]},
        ]},
        {"type": "codeBlock", "attrs": {"language": "rust"}, "content": [t("a\n\tb\n")]},
        {"type": "paragraph", "content": [{"type": "text", "text": "c()", "marks": [{"type": "code"}]}]},
    ]});
    let (ps, _, report) = read(&doc);
    let styles: Vec<&str> = ps.iter().map(style).collect();
    assert_eq!(styles, ["Quote", "Quote", "Heading1", "Normal", "Normal"]);
    assert_eq!(text(&ps[3]), "a\n\tb\n");
    assert_eq!(
        ps[3]
            .inlines
            .iter()
            .filter(|i| matches!(
                i.content,
                InlineContent::Break {
                    kind: BreakKind::Line
                }
            ))
            .count(),
        2
    );
    for p in [&ps[3], &ps[4]] {
        let InlineContent::Text(run) = &p.inlines[0].content else {
            panic!("text first")
        };
        assert_eq!(run.properties.font.as_deref(), Some(CODE_FONT));
    }
    let want = [
        ("block quote (read as the Quote style)", Severity::Minor),
        ("nested block quote (flattened)", Severity::Minor),
        (
            "a heading, rule, list or code block inside a block quote (the quote not kept)",
            Severity::Minor,
        ),
        ("a code block's language (dropped)", Severity::Info),
        ("code block (read as monospace paragraph)", Severity::Minor),
        (
            "inline code (read as a monospace run without its shading)",
            Severity::Minor,
        ),
    ];
    let want: Vec<(String, Severity)> = want.iter().map(|(w, s)| ((*w).to_owned(), *s)).collect();
    assert_eq!(losses(&report), want);
}

#[test]
fn unknown_nodes_marks_and_attributes_keep_their_text_and_are_reported() {
    let doc = json!({"type": "doc", "content": [
        {"type": "paragraph", "attrs": {"textAlign": "center", "id": null}, "content": [
            t("a "),
            {"type": "mention", "attrs": {"id": 1}, "content": [t("@bob")]},
            {"type": "text", "text": " link", "marks": [{"type": "link", "attrs": {"href": "x"}}]},
        ]},
        {"type": "taskList", "content": [{"type": "taskItem", "content": [{"type": "paragraph", "content": [t("task")]}]}]},
        {"type": "image", "attrs": {"src": "x"}},
        {"type": "paragraph", "content": [{"type": "paragraph", "content": [t("inner")]}]},
    ]});
    let (ps, _, report) = read(&doc);
    let texts: Vec<String> = ps.iter().map(text).collect();
    assert_eq!(texts, ["a @bob link", "task", "inner"]);
    let want = [
        (
            "an attribute the chat editor did not have: paragraph.textAlign",
            Severity::Minor,
        ),
        (
            "a node of unknown type mention; its text kept",
            Severity::Major,
        ),
        ("a mark of unknown type link (dropped)", Severity::Minor),
        (
            "a node of unknown type taskList; its text kept",
            Severity::Major,
        ),
        (
            "a node of unknown type taskItem; its text kept",
            Severity::Major,
        ),
        (
            "a node of unknown type image; its text kept",
            Severity::Major,
        ),
        (
            "a block inside a paragraph (read as its text)",
            Severity::Minor,
        ),
    ];
    let want: Vec<(String, Severity)> = want.iter().map(|(w, s)| ((*w).to_owned(), *s)).collect();
    assert_eq!(losses(&report), want);
}

#[test]
fn malformed_input_is_read_as_far_as_it_goes_and_only_non_documents_are_refused() {
    assert!(tiptap_to_document("{").is_err());
    assert!(tiptap_to_document("[1]").is_err());
    assert!(tiptap_to_document(r#"{"type": 3}"#).is_err());
    let doc = json!({"type": "doc", "content": [
        7,
        {"type": "paragraph", "content": {"not": "an array"}},
        {"type": "text"},
        t("loose"),
        {"type": "listItem", "content": [{"type": "paragraph", "content": [t("stray item")]}]},
    ]});
    let (ps, _, report) = read(&doc);
    let texts: Vec<String> = ps.iter().map(text).collect();
    assert_eq!(texts, ["", "loose", "stray item"]);
    assert_eq!(report.count_of("a malformed node (skipped)"), 3);
    let (ps, _, report) = read(&json!({"type": "paragraph", "content": [t("root")]}));
    assert_eq!(ps.iter().map(text).collect::<Vec<_>>(), ["root"]);
    assert_eq!(
        report.count_of("a document whose root is not a doc node (read as its content)"),
        1
    );
}

#[test]
fn an_empty_document_is_the_blank_one() {
    let (doc, report) = tiptap_to_document(r#"{"type":"doc","content":[]}"#).unwrap();
    assert_eq!(doc, cdoc_model::Document::blank());
    assert_eq!(report.count_at_least(Severity::Minor), 0);
    let (doc, report) =
        tiptap_to_document(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#).unwrap();
    assert_eq!(doc, cdoc_model::Document::blank());
    assert!(report.is_lossless());
}

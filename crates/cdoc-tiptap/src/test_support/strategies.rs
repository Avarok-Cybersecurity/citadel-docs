//! Random `StarterKit` trees, as the chat editor saves them, and the same trees with things the
//! editor never had mixed in (unknown nodes, marks and attributes). Boxed at every level:
//! unboxed nested strategies overflow the test stack.

use proptest::prelude::*;
use serde_json::{json, Value};

/// Text with spaces, tabs, accents, CJK, emoji and right-to-left letters; never empty, as a
/// ProseMirror text node never is.
///
/// # Panics
/// Never: the regex is a constant.
pub fn text() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-zA-Z0-9 \t,.!?é漢😀שלוםمرحبا-]{1,10}")
        .expect("valid regex")
        .boxed()
}

fn code_text() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-z0-9 \t(){};\n]{1,16}")
        .expect("valid regex")
        .boxed()
}

fn marks(extra: bool) -> BoxedStrategy<Vec<Value>> {
    let mut names = vec!["bold", "italic", "strike", "code"];
    if extra {
        names.extend(["link", "highlight", "textStyle"]);
    }
    proptest::sample::subsequence(names.clone(), 0..=names.len())
        .prop_map(|chosen| chosen.into_iter().map(|m| json!({"type": m})).collect())
        .boxed()
}

fn inline(extra: bool) -> BoxedStrategy<Value> {
    let text = (text(), marks(extra))
        .prop_map(|(text, marks)| {
            if marks.is_empty() {
                json!({"type": "text", "text": text})
            } else {
                json!({"type": "text", "text": text, "marks": marks})
            }
        })
        .boxed();
    let hard_break = Just(json!({"type": "hardBreak"})).boxed();
    if !extra {
        return prop_oneof![4 => text, 1 => hard_break].boxed();
    }
    let mention = text
        .clone()
        .prop_map(|t| json!({"type": "mention", "attrs": {"id": "u1"}, "content": [t]}))
        .boxed();
    prop_oneof![4 => text, 1 => hard_break, 1 => mention].boxed()
}

fn inlines(extra: bool) -> BoxedStrategy<Vec<Value>> {
    proptest::collection::vec(inline(extra), 0..4).boxed()
}

fn paragraph(extra: bool) -> BoxedStrategy<Value> {
    inlines(extra)
        .prop_map(|c| json!({"type": "paragraph", "content": c}))
        .boxed()
}

fn leaf_block(extra: bool) -> BoxedStrategy<Value> {
    let heading = (1..=6u8, inlines(extra))
        .prop_map(|(level, c)| json!({"type": "heading", "attrs": {"level": level}, "content": c}))
        .boxed();
    let language = prop_oneof![Just(Value::Null), Just(json!("rust"))].boxed();
    let code = (language, proptest::collection::vec(code_text(), 0..2))
        .prop_map(|(language, lines)| {
            let content: Vec<Value> = lines
                .into_iter()
                .map(|t| json!({"type": "text", "text": t}))
                .collect();
            json!({"type": "codeBlock", "attrs": {"language": language}, "content": content})
        })
        .boxed();
    let rule = Just(json!({"type": "horizontalRule"})).boxed();
    prop_oneof![4 => paragraph(extra), 2 => heading, 1 => code, 1 => rule].boxed()
}

fn list_item(block: BoxedStrategy<Value>, extra: bool) -> BoxedStrategy<Value> {
    (paragraph(extra), proptest::collection::vec(block, 0..2))
        .prop_map(|(first, rest)| {
            let mut content = vec![first];
            content.extend(rest);
            json!({"type": "listItem", "content": content})
        })
        .boxed()
}

fn block(extra: bool) -> BoxedStrategy<Value> {
    leaf_block(extra)
        .prop_recursive(4, 48, 3, move |inner| {
            let items = proptest::collection::vec(list_item(inner.clone(), extra), 1..3).boxed();
            let quote = proptest::collection::vec(inner.clone(), 1..3)
                .prop_map(|c| json!({"type": "blockquote", "content": c}))
                .boxed();
            let bullets = items
                .clone()
                .prop_map(|c| json!({"type": "bulletList", "content": c}))
                .boxed();
            let ordered = (0..5u32, items)
                .prop_map(|(start, c)| {
                    json!({"type": "orderedList", "attrs": {"start": start, "type": null}, "content": c})
                })
                .boxed();
            let mut choices = vec![(1, quote), (2, bullets), (2, ordered)];
            if extra {
                let widget = proptest::collection::vec(inner, 1..3)
                    .prop_map(|c| json!({"type": "taskList", "attrs": {"x": 1}, "content": c}))
                    .boxed();
                choices.push((1, widget));
            }
            proptest::strategy::Union::new_weighted(choices).boxed()
        })
        .boxed()
}

/// A chat document of `StarterKit` nodes only.
pub fn starter_kit_doc() -> BoxedStrategy<Value> {
    proptest::collection::vec(block(false), 0..5)
        .prop_map(|c| json!({"type": "doc", "content": c}))
        .boxed()
}

/// A chat document with unknown nodes, marks and attributes mixed in.
pub fn foreign_doc() -> BoxedStrategy<Value> {
    proptest::collection::vec(block(true), 0..5)
        .prop_map(|c| json!({"type": "doc", "content": c}))
        .boxed()
}

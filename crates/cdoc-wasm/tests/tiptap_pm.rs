//! The chat editor's documents as ProseMirror JSON in the model's schema, to show the original
//! read-only: the same mapper as the migration, accepted by the schema check and the existing
//! ProseMirror path, the same bytes every time, refused only when it is not a document.

use cdoc_tiptap::test_support::strategies::{foreign_doc, starter_kit_doc};
use cdoc_tiptap::test_support::CORPUS;
use cdoc_wasm::facade::check_pm;
use cdoc_wasm::migrate::{pm_to_crdt_snapshot, tiptap_to_crdt_snapshot, tiptap_to_pm};
use proptest::prelude::*;
use serde_json::Value;

const CLIENT: u64 = 0x9E37_79B9_7F4A_7C15;

fn check(json: &str) -> Result<(), TestCaseError> {
    let pm = tiptap_to_pm(json).expect("renders");
    prop_assert_eq!(&tiptap_to_pm(json).expect("renders again"), &pm);
    prop_assert!(check_pm(&pm).is_ok(), "{:?}", check_pm(&pm));
    let read = cdoc_pm::from_pm_json(&pm).expect("the ProseMirror path reads it");
    prop_assert_eq!(
        &read,
        &cdoc_tiptap::tiptap_to_document(json).expect("maps").0
    );
    let shown: Value = serde_json::from_str(&pm).expect("JSON");
    prop_assert_eq!(&shown["type"], "doc");
    prop_assert_eq!(&shown["content"][0]["type"], "section");
    let (from_pm, _) = pm_to_crdt_snapshot(&pm, CLIENT).expect("migrates from pm");
    let (from_chat, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect("migrates");
    prop_assert!(
        from_pm == from_chat,
        "the shown document is not the migrated one"
    );
    Ok(())
}

#[test]
fn the_corpus_renders_as_the_document_it_migrates_to() {
    for (name, json) in CORPUS {
        check(json).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn only_non_documents_are_refused() {
    let err = tiptap_to_pm("{\"type\":").unwrap_err();
    assert!(err.contains("not JSON"), "{err}");
    let err = tiptap_to_pm("\"doc\"").unwrap_err();
    assert!(err.contains("not a TipTap node"), "{err}");
    let pm = tiptap_to_pm(r#"{"type":"mystery","content":[{"type":"text","text":"kept"}]}"#);
    assert!(pm.expect("an unknown root is read").contains("kept"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_documents_render_as_what_they_migrate_to(doc in starter_kit_doc()) {
        check(&doc.to_string())?;
    }

    #[test]
    fn foreign_documents_render_as_what_they_migrate_to(doc in foreign_doc()) {
        check(&doc.to_string())?;
    }
}

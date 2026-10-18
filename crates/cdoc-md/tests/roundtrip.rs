//! What Markdown can hold survives write and read: for documents in Markdown's domain, reading
//! the written text says the same thing, and writing that again changes nothing.

mod common;

use cdoc_md::{document_to_md, md_to_document};
use proptest::prelude::*;

proptest! {
    #[test]
    fn markdown_domain_documents_survive_a_round_trip(doc in common::strategies::document()) {
        let (md, written) = document_to_md(&doc);
        let (back, _) = md_to_document(&md, &common::options());
        prop_assert_eq!(common::project(&back), common::project(&doc), "markdown was:\n{}", md);
        prop_assert!(!written.has(|_| true, |w| w.contains("edge punctuation")), "{:?}", written);
        let (again, _) = document_to_md(&back);
        prop_assert_eq!(&again, &md, "second write differs");
    }
}

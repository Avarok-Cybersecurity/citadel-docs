//! The committed corpus imports with nothing a reader would see lost: its `.docx` and `.md`
//! fixtures read back with no Minor or Major loss (the severity table is `cdoc_model::loss`).

use cdoc_bench::corpus::COMMITTED_PAGE_COUNTS;
use cdoc_bench::metrics::MD_OPTIONS;
use cdoc_docx::{read_docx, MemoryMedia, ReadLimits};
use cdoc_model::{Report, Severity};

fn fixture(pages: u32, ext: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/corpus/{pages}p.{ext}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("a committed fixture")
}

fn visible(report: &Report) -> Vec<String> {
    report
        .losses
        .iter()
        .filter(|l| l.severity() >= Severity::Minor)
        .map(|l| format!("{}: {:?}", l.part(), l.what()))
        .collect()
}

#[test]
fn the_corpus_imports_without_a_visible_loss() {
    for pages in COMMITTED_PAGE_COUNTS {
        let mut media = MemoryMedia::with_prefix("img");
        let docx = read_docx(&fixture(pages, "docx"), &ReadLimits::generous(), &mut media)
            .expect("the docx reads");
        assert_eq!(visible(&docx.report), Vec::<String>::new(), "{pages}p.docx");
        let md = String::from_utf8(fixture(pages, "md")).expect("UTF-8");
        let (_, report) = cdoc_md::md_to_document(&md, &MD_OPTIONS);
        assert_eq!(visible(&report), Vec::<String>::new(), "{pages}p.md");
    }
}

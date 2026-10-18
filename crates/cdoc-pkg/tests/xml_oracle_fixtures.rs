//! Every XML part of every committed fixture (`.docx`, `.dotx`, `.docm`, `.odt` and the bench
//! corpus) parses to the tree the quick-xml parser it replaced builds, in both whitespace modes.

mod oracle;

use cdoc_pkg::xml::{parse, Whitespace};
use cdoc_pkg::{Package, ReadLimits};
use std::path::PathBuf;

const DIRS: [&str; 3] = [
    "../cdoc-docx/tests/fixtures",
    "../cdoc-odt/tests/fixtures",
    "../cdoc-bench/corpus",
];
const PACKAGES: [&str; 4] = [".docx", ".dotx", ".docm", ".odt"];

fn fixtures() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found: Vec<PathBuf> = DIRS
        .iter()
        .flat_map(|d| std::fs::read_dir(root.join(d)).expect("a fixture directory"))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| PACKAGES.iter().any(|x| p.to_string_lossy().ends_with(x)))
        .collect();
    found.sort();
    found
}

fn is_xml(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or_default();
    ext.eq_ignore_ascii_case("xml") || ext.eq_ignore_ascii_case("rels")
}

#[test]
fn every_fixture_part_parses_to_the_oracle_tree() {
    let (mut packages, mut parts) = (0, 0);
    for path in fixtures() {
        let bytes = std::fs::read(&path).expect("a fixture");
        let package = Package::read(&bytes, &ReadLimits::generous()).expect("a fixture archive");
        packages += 1;
        for name in package.names().filter(|n| is_xml(n)) {
            let xml = package.get(name).expect("a listed part");
            for ws in [Whitespace::DropBetweenElements, Whitespace::Keep] {
                let ours = parse(name, xml, ws, &ReadLimits::generous()).map_err(|e| e.to_string());
                let theirs = oracle::parse(xml, ws);
                match (&ours, &theirs) {
                    (Ok(a), Ok(b)) => assert_eq!(a, b, "{} {name} {ws:?}", path.display()),
                    (Err(_), Err(_)) => {}
                    _ => panic!("{} {name} {ws:?}: {ours:?} vs {theirs:?}", path.display()),
                }
            }
            parts += 1;
        }
    }
    assert_eq!(
        (packages, parts),
        (11, 94),
        "fixtures went missing or appeared"
    );
}

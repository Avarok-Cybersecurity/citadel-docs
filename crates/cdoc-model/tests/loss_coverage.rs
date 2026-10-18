//! Every loss a reader or writer can record is named by the severity table: each sentence
//! passed to `lose`, `unknown`, `note` or `note_n` in the workspace's sources, literal or
//! `format!`ted (its placeholders filled with an element name), classifies as something other
//! than the unclassified fallback. A new loss sentence without a row fails here, not in a
//! user's report as an unexplained Major.

use cdoc_model::loss::{classify, labels};
use std::fs;
use std::path::{Path, PathBuf};

/// The argument index holding the `what` for each recording call.
const CALLS: &[(&str, usize)] = &[
    (".lose(", 0),
    (".unknown(", 0),
    (".note(", 1),
    (".note_n(", 1),
];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The top-level arguments of the call whose `(` ends at `from`.
fn arguments(s: &str, from: usize) -> Vec<String> {
    let (mut depth, mut start, mut quoted, mut escaped) = (1, from, false, false);
    let mut out = Vec::new();
    for (i, c) in s[from..].char_indices().map(|(i, c)| (i + from, c)) {
        if quoted {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(s[start..i].trim().to_owned());
                    return out;
                }
            }
            ',' if depth == 1 => {
                out.push(s[start..i].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

/// The sentence an argument records: a literal, or a `format!` with every `{..}` filled.
fn sentence(arg: &str) -> Option<String> {
    let body = arg
        .strip_prefix("&format!(")
        .and_then(|a| a.strip_suffix(')'))
        .map_or(arg, str::trim);
    let literal = body.strip_prefix('"')?;
    let literal = &literal[..literal.find('"')?];
    let mut filled = String::new();
    let mut rest = literal;
    while let Some(open) = rest.find('{') {
        filled.push_str(&rest[..open]);
        let close = rest[open..].find('}')? + open;
        filled.push_str("x:value");
        rest = &rest[close + 1..];
    }
    filled.push_str(rest);
    Some(filled)
}

#[test]
fn every_recorded_loss_is_named_by_the_table() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in fs::read_dir(&crates).expect("the crates") {
        let src = entry.expect("a crate").path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    let mut checked = 0;
    let mut unnamed = Vec::new();
    for file in files {
        if file.ends_with("cdoc-model/src/report.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("a source file");
        for &(call, index) in CALLS {
            for (at, _) in text.match_indices(call) {
                let args = arguments(&text, at + call.len());
                let Some(what) = args.get(index).and_then(|a| sentence(a)) else {
                    continue;
                };
                checked += 1;
                if classify("x:part", &what).label == labels::UNCLASSIFIED {
                    unnamed.push(format!("{}: {what}", file.display()));
                }
            }
        }
    }
    assert!(
        checked > 200,
        "the scan found only {checked} recording calls"
    );
    assert!(
        unnamed.is_empty(),
        "losses the table does not name:\n{}",
        unnamed.join("\n")
    );
}

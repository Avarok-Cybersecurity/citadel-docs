//! Parts built to break or stall a parser. Each is an error naming the problem, or a tree, in
//! time linear in the part, and never a panic.

use cdoc_pkg::xml::{parse_tree, Whitespace};
use cdoc_pkg::{PackageError, ReadLimits};
use std::fmt::Write;
use std::time::{Duration, Instant};

fn read(text: &str) -> Result<usize, String> {
    read_with(text.as_bytes(), &ReadLimits::generous())
}

fn read_with(bytes: &[u8], limits: &ReadLimits) -> Result<usize, String> {
    match parse_tree("p", bytes, Whitespace::DropBetweenElements, limits) {
        Ok(tree) => Ok(tree.root().elems().count()),
        Err(PackageError::Xml { detail, .. }) => Err(detail),
        Err(other) => panic!("not an XML error: {other:?}"),
    }
}

fn nested(depth: usize) -> String {
    format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth))
}

#[test]
fn nesting_is_bounded_by_the_limit_and_never_by_the_stack() {
    let limits = ReadLimits::generous();
    assert_eq!(read(&nested(limits.max_depth)), Ok(1));
    let too_deep = read(&nested(limits.max_depth + 1)).expect_err("one level past the limit");
    assert!(too_deep.contains("nested deeper"), "{too_deep}");
    let started = Instant::now();
    assert!(read(&nested(1_000_000)).is_err());
    assert!(started.elapsed() < Duration::from_secs(5), "fails fast");
    let lax = ReadLimits {
        max_depth: usize::MAX,
        ..limits
    };
    assert_eq!(
        read_with(nested(200_000).as_bytes(), &lax),
        Ok(1),
        "built without recursion"
    );
}

fn element_with_attributes(count: usize, duplicate_of: Option<usize>) -> String {
    let mut text = String::from("<a");
    for i in 0..count {
        let _ = write!(text, " k{i}=\"v\"");
    }
    if let Some(i) = duplicate_of {
        let _ = write!(text, " k{i}=\"again\"");
    }
    text.push_str("/>");
    text
}

#[test]
fn duplicate_attributes_are_found_wherever_they_are_in_linearithmic_time() {
    for count in [1, 2, 15, 16, 17, 40] {
        assert!(
            read(&element_with_attributes(count, None)).is_ok(),
            "{count}"
        );
        for dup in [0, count / 2, count - 1] {
            let err = read(&element_with_attributes(count, Some(dup))).expect_err("a duplicate");
            assert_eq!(err, format!("duplicate attribute k{dup}"), "{count} {dup}");
        }
    }
    let started = Instant::now();
    assert!(read(&element_with_attributes(200_000, None)).is_ok());
    assert!(read(&element_with_attributes(200_000, Some(0))).is_err());
    assert!(started.elapsed() < Duration::from_secs(5), "not quadratic");
}

#[test]
fn declared_entities_are_refused_not_expanded() {
    let mut laughs = String::from("<!DOCTYPE a [<!ENTITY l0 \"lol\">");
    for i in 1..10 {
        let _ = write!(
            laughs,
            "<!ENTITY l{i} \"{}\">",
            format!("&l{};", i - 1).repeat(10)
        );
    }
    laughs.push_str("]><a>&l9;</a>");
    assert_eq!(read(&laughs), Err("unknown entity &l9;".to_owned()));
    let external = "<!DOCTYPE a SYSTEM \"file:///etc/passwd\"><a>&ext;</a>";
    assert_eq!(read(external), Err("unknown entity &ext;".to_owned()));
}

#[test]
fn markup_xml_forbids_is_refused() {
    for (text, why) in [
        ("<a b=\"<\"/>", "holds a '<'"),
        ("<a\"b/>", "without a value"),
        ("<a b'c=\"1\"/>", "without a value"),
        ("x<a/>", "outside the document element"),
        ("<a/>x", "outside the document element"),
        ("&#0;<a/>", "outside the document element"),
        ("<?><a/>", "without a target"),
        ("<!DOCTYPE><a/>", "without a name"),
        ("<!DOCTYPE <a/>", "without a name"),
        ("<!DOCTYPE a<a/>", "unterminated document type"),
        (
            "<!DOCTYPE a [ ]x><a/>",
            "not closed after its internal subset",
        ),
        ("<a/><b/>", "after the document element"),
        ("<a>&#0;</a>", "invalid character reference"),
        ("<a>&#xD800;</a>", "invalid character reference"),
    ] {
        let err = read(text).expect_err(text);
        assert!(err.contains(why), "{text}: {err}");
    }
    assert_eq!(
        read(" <?xml version=\"1.0\"?>\n<!DOCTYPE a [ ] >\n<a/>\n"),
        Ok(0)
    );
}

#[test]
fn bad_utf8_and_every_truncation_are_errors_not_panics() {
    let text = "<?xml version=\"1.0\"?><w:p a='1' b=\"&amp;\"><!--c--><![CDATA[x]]>caf\u{e9}\
                &#x1F600;<?pi d?><w:t/></w:p>";
    assert_eq!(read(text), Ok(1));
    for len in 0..text.len() {
        assert!(
            read_with(&text.as_bytes()[..len], &ReadLimits::generous()).is_err(),
            "{len}"
        );
    }
    let mut bad = text.as_bytes().to_vec();
    let at = text.find('\u{e9}').expect("an accent");
    bad[at + 1] = 0xFF;
    assert!(read_with(&bad, &ReadLimits::generous())
        .expect_err("bad UTF-8")
        .contains("utf-8"));
}

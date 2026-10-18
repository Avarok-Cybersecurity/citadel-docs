//! Date pictures (`d MMMM yyyy`, Word's spelling, which the model keeps) and ODF date styles
//! (`number:date-style`). A picture is cut into maximal runs of one letter; a run ODF has an
//! element for becomes that element, anything else becomes literal `number:text`. Reading
//! concatenates the pieces back, so every picture survives exactly.

use cdoc_pkg::xml::Node;
use cdoc_pkg::Log;

/// Element, attributes, and the picture run each one is.
/// An element, the attributes it must carry, and the picture run it is.
type Token = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static str,
);

const TOKENS: [Token; 17] = [
    ("number:day", &[], "d"),
    ("number:day", &[("number:style", "long")], "dd"),
    ("number:day-of-week", &[], "ddd"),
    ("number:day-of-week", &[("number:style", "long")], "dddd"),
    ("number:month", &[], "M"),
    ("number:month", &[("number:style", "long")], "MM"),
    ("number:month", &[("number:textual", "true")], "MMM"),
    (
        "number:month",
        &[("number:textual", "true"), ("number:style", "long")],
        "MMMM",
    ),
    ("number:year", &[], "yy"),
    ("number:year", &[("number:style", "long")], "yyyy"),
    ("number:hours", &[], "H"),
    ("number:hours", &[("number:style", "long")], "HH"),
    ("number:minutes", &[], "m"),
    ("number:minutes", &[("number:style", "long")], "mm"),
    ("number:seconds", &[], "s"),
    ("number:seconds", &[("number:style", "long")], "ss"),
    ("number:era", &[], "gg"),
];

fn runs(picture: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in picture.chars() {
        match out.last_mut() {
            Some(last) if last.starts_with(c) && c.is_ascii_alphabetic() => last.push(c),
            _ => out.push(c.to_string()),
        }
    }
    out
}

/// The date style for a picture.
#[must_use]
pub fn write_date_style(name: &str, picture: &str) -> Node {
    let mut style = Node::new("number:date-style").attr("style:name", name);
    let mut literal = String::new();
    let flush = |style: Node, literal: &mut String| {
        if literal.is_empty() {
            style
        } else {
            style.child(Node::new("number:text").text(&std::mem::take(literal)))
        }
    };
    for run in runs(picture) {
        if let Some((element, attrs, _)) = TOKENS.iter().find(|(_, _, t)| *t == run) {
            style = flush(style, &mut literal);
            let mut node = Node::new(element);
            for (k, v) in *attrs {
                node = node.attr(k, v);
            }
            style = style.child(node);
        } else {
            literal.push_str(&run);
        }
    }
    flush(style, &mut literal)
}

/// The picture a date style spells.
pub fn read_date_style(style: &Node, log: &mut Log) -> String {
    let mut picture = String::new();
    for child in style.elems() {
        if child.name == "number:text" {
            picture.push_str(&child.content());
            continue;
        }
        let token = TOKENS.iter().find(|(element, attrs, _)| {
            *element == child.name
                && attrs.iter().all(|(k, v)| child.get(k) == Some(*v))
                && child
                    .attrs
                    .iter()
                    .filter(|(k, _)| k == "number:style" || k == "number:textual")
                    .all(|(k, v)| {
                        attrs.contains(&(k.as_str(), v.as_str()))
                            || (k == "number:style" && v == "short")
                    })
        });
        match token {
            Some((_, _, run)) => picture.push_str(run),
            None => log.lose(&format!("{}/{}", style.name, child.name)),
        }
    }
    picture
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(picture: &str) -> String {
        let mut log = Log::new("t");
        let back = read_date_style(&write_date_style("N1", picture), &mut log);
        assert!(log.report.is_lossless(), "{:?}", log.report);
        back
    }

    #[test]
    fn every_picture_comes_back_as_written() {
        for p in [
            "d MMMM yyyy",
            "yyyy-MM-dd",
            "dddd, d. MMM yy",
            "HH:mm:ss",
            "ddddd q",
            "'at' h",
            "",
        ] {
            assert_eq!(round_trip(p), p);
        }
    }

    #[test]
    fn tokens_become_odf_elements() {
        let style = write_date_style("N1", "d MMMM yyyy");
        let names: Vec<&str> = style.elems().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "number:day",
                "number:text",
                "number:month",
                "number:text",
                "number:year"
            ]
        );
        assert_eq!(
            style.elems().nth(2).and_then(|m| m.get("number:textual")),
            Some("true")
        );
    }

    #[test]
    fn libreoffice_short_styles_and_unknown_elements() {
        let style = Node::new("number:date-style")
            .child(Node::new("number:day").attr("number:style", "short"))
            .child(Node::new("number:quarter"));
        let mut log = Log::new("t");
        assert_eq!(read_date_style(&style, &mut log), "d");
        assert_eq!(log.report.count_of("number:date-style/number:quarter"), 1);
    }
}

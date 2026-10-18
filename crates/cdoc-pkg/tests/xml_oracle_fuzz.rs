//! The lexer against the quick-xml parser it replaced, on generated XML. Well-formed text
//! written every way the syntax allows (either quote, references, CDATA, comments, processing
//! instructions, a prolog, empty-element tags) must give the same tree. Damaged text may be
//! refused where the oracle was lenient, but never accepted where the oracle refuses it, and
//! never read differently.

mod oracle;

use cdoc_pkg::xml::{parse, Child, Node, Whitespace};
use cdoc_pkg::ReadLimits;
use proptest::prelude::*;
use std::fmt::Write;

const NAMES: [&str; 5] = ["w:p", "w:r", "w:t", "a", "text:span"];
const KEYS: [&str; 4] = ["w:val", "id", "x:y", "k"];
const CHARS: &str = "[a-z <>&\"' \t\né😀\u{FFFD}]{0,10}";
const SNIPPETS: [&str; 18] = [
    "<",
    ">",
    "&",
    "&amp;",
    "&#x1F600;",
    "&#0;",
    "&bogus;",
    "<![CDATA[x<y]]>",
    "<!-- c -->",
    "<?pi x?>",
    "</w:p>",
    "<w:t>",
    " a=\"1\"",
    " a='1' a='2'",
    "\"",
    "'",
    "/>",
    "]]>",
];

fn tree() -> impl Strategy<Value = Node> {
    let leaf = (
        0..NAMES.len(),
        proptest::collection::btree_map(0..KEYS.len(), CHARS, 0..3),
    )
        .prop_map(|(n, attrs)| {
            let mut node = Node::new(NAMES[n]);
            node.attrs = attrs
                .into_iter()
                .map(|(k, v)| (KEYS[k].to_owned(), v))
                .collect();
            node
        });
    leaf.prop_recursive(4, 40, 4, |inner| {
        (
            (0..NAMES.len()),
            proptest::collection::vec(
                prop_oneof![inner.prop_map(Child::Node), CHARS.prop_map(Child::Text)],
                0..4,
            ),
        )
            .prop_map(|(n, children)| Node {
                name: NAMES[n].to_owned(),
                attrs: Vec::new(),
                children,
            })
    })
}

/// Choices for the writer, consumed in order and reused cyclically.
struct Style<'a>(&'a [u8], usize);

impl Style<'_> {
    fn pick(&mut self, of: u8) -> u8 {
        let v = self
            .0
            .get(self.1 % self.0.len().max(1))
            .copied()
            .unwrap_or(0);
        self.1 += 1;
        v % of
    }
}

fn escape(text: &str, quote: Option<char>, style: &mut Style<'_>, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' if style.pick(2) == 0 => out.push_str("&gt;"),
            c if Some(c) == quote => out.push_str(if c == '"' { "&quot;" } else { "&apos;" }),
            '\t' | '\n' | '\r' if quote.is_some() => {
                let _ = write!(out, "&#{};", u32::from(c));
            }
            c if c != ' ' && style.pick(4) == 0 => {
                let _ = if style.pick(2) == 0 {
                    write!(out, "&#{};", u32::from(c))
                } else {
                    write!(out, "&#x{:X};", u32::from(c))
                };
            }
            c => out.push(c),
        }
    }
}

fn write_node(node: &Node, style: &mut Style<'_>, out: &mut String) {
    out.push('<');
    out.push_str(&node.name);
    for (k, v) in &node.attrs {
        let quote = if style.pick(2) == 0 { '"' } else { '\'' };
        let pad = [" ", "  ", "\n "][usize::from(style.pick(3))];
        let before = ["", " "][usize::from(style.pick(2))];
        let after = ["", " "][usize::from(style.pick(2))];
        let _ = write!(out, "{pad}{k}{before}={after}{quote}");
        escape(v, Some(quote), style, out);
        out.push(quote);
    }
    if node.children.is_empty() && style.pick(2) == 0 {
        out.push_str(["/>", " />"][usize::from(style.pick(2))]);
        return;
    }
    out.push_str([">", " >"][usize::from(style.pick(2))]);
    for child in &node.children {
        match style.pick(5) {
            0 => out.push_str("<!--x-->"),
            1 => out.push_str("<?p d?>"),
            _ => {}
        }
        match child {
            Child::Node(n) => write_node(n, style, out),
            Child::Text(t) if !t.contains("]]>") && style.pick(4) == 0 => {
                let _ = write!(out, "<![CDATA[{t}]]>");
            }
            Child::Text(t) => escape(t, None, style, out),
        }
    }
    let _ = write!(
        out,
        "</{}{}>",
        node.name,
        ["", " "][usize::from(style.pick(2))]
    );
}

fn document(root: &Node, style: &[u8]) -> String {
    let mut style = Style(style, 0);
    let mut out = String::new();
    for prolog in [
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE a>",
        "<!-- p -->",
        "\n",
    ] {
        if style.pick(2) == 0 {
            out.push_str(prolog);
        }
    }
    write_node(root, &mut style, &mut out);
    out
}

fn both(bytes: &[u8], ws: Whitespace) -> (Result<Node, String>, Result<Node, String>) {
    let ours = parse("p", bytes, ws, &ReadLimits::generous()).map_err(|e| e.to_string());
    (ours, oracle::parse(bytes, ws))
}

const MODES: [Whitespace; 2] = [Whitespace::DropBetweenElements, Whitespace::Keep];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn well_formed_text_reads_as_the_oracle_reads_it(
        root in tree(),
        style in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        let text = document(&root, &style);
        for ws in MODES {
            let (ours, theirs) = both(text.as_bytes(), ws);
            prop_assert!(theirs.is_ok(), "the generator wrote what the oracle refuses: {theirs:?}\n{text}");
            prop_assert_eq!(ours, theirs, "{}", text);
        }
    }

    #[test]
    fn damaged_text_is_never_accepted_where_the_oracle_refuses_it(
        root in tree(),
        style in proptest::collection::vec(any::<u8>(), 1..64),
        edits in proptest::collection::vec((0..3u8, any::<prop::sample::Index>(), 0..SNIPPETS.len(), 1..20usize), 1..4),
    ) {
        let mut bytes = document(&root, &style).into_bytes();
        for (op, at, snippet, len) in edits {
            let at = at.index(bytes.len() + 1);
            match op {
                0 => drop(bytes.splice(at..at, SNIPPETS[snippet].bytes())),
                1 => drop(bytes.drain(at..(at + len).min(bytes.len()))),
                _ => {
                    let copy = bytes[at..(at + len).min(bytes.len())].to_vec();
                    drop(bytes.splice(at..at, copy));
                }
            }
        }
        for ws in MODES {
            if let (Ok(ours), theirs) = both(&bytes, ws) {
                prop_assert_eq!(Ok(ours), theirs, "{}", String::from_utf8_lossy(&bytes));
            }
        }
    }
}

//! Losses of reading the chat editor's documents (TipTap `StarterKit` JSON, `cdoc-tiptap`):
//! the constructs the model has no node for, and input that is not the editor's.

use super::labels::{CONTENT_NOT_KEPT, FORMAT_APPROXIMATED, FORMAT_NOT_KEPT, STRUCTURE_SIMPLIFIED};
use super::Severity;

const I: Severity = Severity::Info;
const MI: Severity = Severity::Minor;
const MA: Severity = Severity::Major;

/// Sentences, exactly as `cdoc-tiptap` records them.
#[rustfmt::skip]
pub(super) const EXACT: &[(&str, Severity, &str)] = &[
    ("a code block's language (dropped)", I, FORMAT_NOT_KEPT),
    ("a heading level outside 1 to 6 read as the nearest", MI, FORMAT_APPROXIMATED),
    ("block quote (read as the Quote style)", MI, FORMAT_APPROXIMATED),
    ("inline code (read as a monospace run without its shading)", MI, FORMAT_APPROXIMATED),
    ("a list start below 0 or not a whole number (read as 1)", MI, FORMAT_APPROXIMATED),
    ("a heading, rule, list or code block inside a block quote (the quote not kept)", MI, STRUCTURE_SIMPLIFIED),
    ("a heading, rule or code block inside a list item (read outside the list)", MI, STRUCTURE_SIMPLIFIED),
    ("a block quote inside a list item (the quote not kept)", MI, STRUCTURE_SIMPLIFIED),
    ("a list item without a first paragraph (an empty one added)", MI, STRUCTURE_SIMPLIFIED),
    ("a list item outside a list (read as its paragraphs)", MI, STRUCTURE_SIMPLIFIED),
    ("a block inside a paragraph (read as its text)", MI, STRUCTURE_SIMPLIFIED),
    ("a document whose root is not a doc node (read as its content)", MI, STRUCTURE_SIMPLIFIED),
    ("a malformed node (skipped)", MA, CONTENT_NOT_KEPT),
];

/// Sentences with a value (a node, mark or attribute name) between `start` and `end`.
#[rustfmt::skip]
pub(super) const PATTERNS: &[(&str, &str, Severity, &str)] = &[
    ("a node of unknown type ", "; its text kept", MA, CONTENT_NOT_KEPT),
    ("a mark of unknown type ", " (dropped)", MI, FORMAT_NOT_KEPT),
    ("an attribute the chat editor did not have: ", "", MI, FORMAT_NOT_KEPT),
];

//! String ids in an integer format. OOXML numbers comments and tracked changes; the model
//! names them with strings. The writer numbers each distinct string once and records the pairs
//! in a small sidecar part; the reader looks the numbers up again, and for a file Word wrote
//! (no sidecar) uses the number itself as the id.

use crate::error::DocxError;
use crate::xml::{self, Node};

/// The sidecar part's name.
pub const PART: &str = "cdoc/ids.xml";
/// The sidecar part's content type.
pub const CONTENT_TYPE: &str = "application/vnd.avarok.cdoc-ids+xml";
const NS: &str = "https://citadel-docs.avarok.net/ns/ids";

/// Ids of one kind, numbered by first appearance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdTable {
    names: Vec<String>,
}

impl IdTable {
    /// An empty table.
    #[must_use]
    pub const fn empty() -> Self {
        Self { names: Vec::new() }
    }

    /// The number for an id, assigning the next one on first sight.
    pub fn number(&mut self, id: &str) -> usize {
        if let Some(n) = self.names.iter().position(|n| n == id) {
            return n;
        }
        self.names.push(id.to_owned());
        self.names.len() - 1
    }

    /// The id for a number; the number itself when the table has no such entry.
    #[must_use]
    pub fn name(&self, number: usize) -> String {
        self.names
            .get(number)
            .cloned()
            .unwrap_or_else(|| number.to_string())
    }
}

/// Both tables, as stored in the sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ids {
    /// Comment ids.
    pub comments: IdTable,
    /// Tracked-change ids.
    pub revisions: IdTable,
}

impl Ids {
    /// Empty tables.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            comments: IdTable::empty(),
            revisions: IdTable::empty(),
        }
    }

    /// The sidecar part, or `None` when there is nothing to record.
    #[must_use]
    pub fn to_node(&self) -> Option<Node> {
        if self.comments.names.is_empty() && self.revisions.names.is_empty() {
            return None;
        }
        let entries = |kind: &str, table: &IdTable| -> Vec<Node> {
            table
                .names
                .iter()
                .enumerate()
                .map(|(n, id)| Node::new(kind).num("n", n).attr("id", id))
                .collect()
        };
        Some(
            Node::new("ids")
                .attr("xmlns", NS)
                .children(entries("comment", &self.comments))
                .children(entries("revision", &self.revisions)),
        )
    }

    /// Parse the sidecar.
    pub fn parse(bytes: &[u8]) -> Result<Self, DocxError> {
        let tree = xml::parse(PART, bytes)?;
        let mut ids = Self::empty();
        for entry in tree.root().elems() {
            let table = match entry.name() {
                "comment" => &mut ids.comments,
                "revision" => &mut ids.revisions,
                _ => continue,
            };
            let (Some(n), Some(id)) = (
                entry.get("n").and_then(|n| n.parse::<usize>().ok()),
                entry.get("id"),
            ) else {
                continue;
            };
            if table.names.len() <= n {
                table.names.resize(n + 1, String::new());
            }
            id.clone_into(&mut table.names[n]);
        }
        Ok(ids)
    }
}

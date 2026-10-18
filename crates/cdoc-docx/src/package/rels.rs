//! Relationships: how one part names another.

use super::rel_type;
use crate::error::DocxError;
use crate::xml::{self, Node};

/// One relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rel {
    /// The id parts refer to (`rId1`).
    pub id: String,
    /// The full type URI.
    pub kind: String,
    /// The target: a part path, or a URL when external.
    pub target: String,
    /// Whether the target is outside the package.
    pub external: bool,
}

/// The relationships of one part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rels {
    items: Vec<Rel>,
}

impl Rels {
    /// No relationships.
    #[must_use]
    pub const fn empty() -> Self {
        Self { items: Vec::new() }
    }

    /// Parse a rels part.
    pub fn parse(part: &str, bytes: &[u8]) -> Result<Self, DocxError> {
        let tree = xml::parse(part, bytes)?;
        let items = tree.root()
            .elems()
            .filter(|n| n.name() == "Relationship")
            .filter_map(|n| {
                Some(Rel {
                    id: n.get("Id")?.to_owned(),
                    kind: n.get("Type")?.to_owned(),
                    target: n.get("Target")?.to_owned(),
                    external: n.get("TargetMode") == Some("External"),
                })
            })
            .collect();
        Ok(Self { items })
    }

    /// The relationship with this id.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&Rel> {
        self.items.iter().find(|r| r.id == id)
    }

    /// The first relationship of a type (short name or URI).
    #[must_use]
    pub fn of_type(&self, kind: &str) -> Option<&Rel> {
        let uri = rel_type::uri(kind);
        self.items.iter().find(|r| r.kind == uri)
    }

    /// Add a relationship, reusing an identical one; returns its id.
    pub fn add(&mut self, kind: &str, target: &str, external: bool) -> String {
        let uri = rel_type::uri(kind);
        if let Some(found) = self
            .items
            .iter()
            .find(|r| r.kind == uri && r.target == target && r.external == external)
        {
            return found.id.clone();
        }
        let id = format!("rId{}", self.items.len() + 1);
        self.items.push(Rel {
            id: id.clone(),
            kind: uri,
            target: target.to_owned(),
            external,
        });
        id
    }

    /// Whether there is nothing to write.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The rels part as XML.
    #[must_use]
    pub fn to_node(&self) -> Node {
        let ns = "http://schemas.openxmlformats.org/package/2006/relationships";
        Node::new("Relationships")
            .attr("xmlns", ns)
            .children(self.items.iter().map(|r| {
                let node = Node::new("Relationship")
                    .attr("Id", &r.id)
                    .attr("Type", &r.kind)
                    .attr("Target", &r.target);
                if r.external {
                    node.attr("TargetMode", "External")
                } else {
                    node
                }
            }))
    }
}

//! The sidecar part: model values ODF has no slot for, keyed by the ODF construct that holds
//! the rest of the value (a style name, an annotation name, a change region), so a document
//! survives `model -> odt -> model` unchanged. Other ODF readers ignore the part; what it holds
//! is listed in the writer's report.

use crate::residual::Residual;
use cdoc_model::{BreakKind, CommentId, FootnoteId, Marks, NumId, NumberingInstance, RevisionId};
use cdoc_pkg::xml::{self, Node, Whitespace};
use cdoc_pkg::PackageError;
use std::collections::{BTreeMap, BTreeSet};

/// The part's name.
pub const PART: &str = "cdoc/sidecar.xml";
/// Its media type, as listed in the manifest.
pub const MEDIA_TYPE: &str = "application/vnd.avarok.cdoc-odt-sidecar+xml";
const NS: &str = "https://citadel-docs.avarok.net/ns/odt-sidecar";

/// A comment's values with no ODF 1.2 slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentExtra {
    /// The comment.
    pub id: CommentId,
    /// Whether the thread is resolved.
    pub resolved: bool,
    /// The comment replied to.
    pub parent: Option<CommentId>,
}

/// Everything the sidecar holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sidecar {
    /// Corrections to what a style (by family and name) reads as.
    pub residuals: BTreeMap<(String, String), Residual>,
    /// Paragraph styles that mark the second half of a paragraph split at a break, with the
    /// break's kind and marks.
    pub continuations: BTreeMap<String, (BreakKind, Marks)>,
    /// Cell styles that mark cells added to fill a ragged row.
    pub pads: BTreeSet<String>,
    /// Sections whose first-page header and footer live in an unreferenced master page.
    pub first_masters: BTreeMap<usize, String>,
    /// The list instances, when the document has any.
    pub instances: Option<Vec<NumberingInstance>>,
    /// The instance each top-level `text:list` (by `xml:id`) belongs to.
    pub lists: BTreeMap<String, NumId>,
    /// Comments in model order, with what ODF cannot say about them.
    pub comments: Vec<CommentExtra>,
    /// Annotations that are a further range of an earlier comment.
    pub segments: BTreeMap<String, CommentId>,
    /// The revision each change region belongs to.
    pub regions: BTreeMap<String, RevisionId>,
    /// Footnotes in model order.
    pub footnotes: Vec<FootnoteId>,
    /// Footnotes the body never refers to (written at a placeholder position).
    pub orphans: BTreeSet<FootnoteId>,
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("model values serialise")
}

impl Sidecar {
    /// Nothing recorded.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            residuals: BTreeMap::new(),
            continuations: BTreeMap::new(),
            pads: BTreeSet::new(),
            first_masters: BTreeMap::new(),
            instances: None,
            lists: BTreeMap::new(),
            comments: Vec::new(),
            segments: BTreeMap::new(),
            regions: BTreeMap::new(),
            footnotes: Vec::new(),
            orphans: BTreeSet::new(),
        }
    }

    /// The part.
    ///
    /// # Panics
    /// Never: model values always serialise.
    #[must_use]
    pub fn to_node(&self) -> Node {
        let mut root = Node::new("sidecar").attr("xmlns", NS).attr("version", "1");
        for ((family, name), r) in &self.residuals {
            let entry = Node::new("residual")
                .attr("family", family)
                .attr("name", name);
            root = root.child(entry.text(&r.to_json()));
        }
        for (style, (kind, marks)) in &self.continuations {
            let entry = Node::new("continuation")
                .attr("style", style)
                .attr("kind", &json(kind));
            root = root.child(entry.text(&json(marks)));
        }
        root = root.children(self.pads.iter().map(|s| Node::new("pad").attr("style", s)));
        for (section, master) in &self.first_masters {
            root = root.child(
                Node::new("first")
                    .num("section", section)
                    .attr("master", master),
            );
        }
        if let Some(instances) = &self.instances {
            root = root.child(Node::new("instances").text(&json(instances)));
        }
        for (id, num) in &self.lists {
            root = root.child(Node::new("list").attr("id", id).num("num", num.0));
        }
        for c in &self.comments {
            let mut entry = Node::new("comment")
                .attr("id", c.id.as_str())
                .attr("resolved", if c.resolved { "true" } else { "false" });
            if let Some(p) = &c.parent {
                entry = entry.attr("parent", p.as_str());
            }
            root = root.child(entry);
        }
        for (name, id) in &self.segments {
            root = root.child(
                Node::new("segment")
                    .attr("name", name)
                    .attr("comment", id.as_str()),
            );
        }
        for (region, id) in &self.regions {
            root = root.child(
                Node::new("region")
                    .attr("id", region)
                    .attr("revision", id.as_str()),
            );
        }
        for id in &self.footnotes {
            let orphan = if self.orphans.contains(id) {
                "true"
            } else {
                "false"
            };
            root = root.child(Node::new("footnote").num("id", id.0).attr("orphan", orphan));
        }
        root
    }

    /// Parse the part.
    pub fn parse(bytes: &[u8]) -> Result<Self, PackageError> {
        let root = xml::parse(PART, bytes, Whitespace::Keep)?;
        let bad = |detail: String| PackageError::Invalid {
            part: PART.to_owned(),
            detail,
        };
        let attr = |n: &Node, key: &str| -> Result<String, PackageError> {
            n.get(key)
                .map(str::to_owned)
                .ok_or_else(|| bad(format!("{} without {key}", n.name)))
        };
        let number = |n: &Node, key: &str| -> Result<u32, PackageError> {
            attr(n, key)?
                .parse()
                .map_err(|_| bad(format!("{} with a bad {key}", n.name)))
        };
        let from_json = |text: &str| bad(format!("bad JSON in the sidecar: {text}"));
        let mut s = Self::empty();
        for n in root.elems() {
            match n.name.as_str() {
                "residual" => {
                    let r = Residual::from_json(&n.content()).map_err(|e| from_json(&e))?;
                    s.residuals
                        .insert((attr(n, "family")?, attr(n, "name")?), r);
                }
                "continuation" => {
                    let kind = serde_json::from_str(&attr(n, "kind")?)
                        .map_err(|e| from_json(&e.to_string()))?;
                    let marks = serde_json::from_str(&n.content())
                        .map_err(|e| from_json(&e.to_string()))?;
                    s.continuations.insert(attr(n, "style")?, (kind, marks));
                }
                "pad" => {
                    s.pads.insert(attr(n, "style")?);
                }
                "first" => {
                    let section =
                        usize::try_from(number(n, "section")?).map_err(|e| bad(e.to_string()))?;
                    s.first_masters.insert(section, attr(n, "master")?);
                }
                "instances" => {
                    s.instances = Some(
                        serde_json::from_str(&n.content())
                            .map_err(|e| from_json(&e.to_string()))?,
                    );
                }
                "list" => {
                    s.lists.insert(attr(n, "id")?, NumId(number(n, "num")?));
                }
                "comment" => s.comments.push(CommentExtra {
                    id: CommentId::new(attr(n, "id")?),
                    resolved: n.get("resolved") == Some("true"),
                    parent: n.get("parent").map(CommentId::new),
                }),
                "segment" => {
                    s.segments
                        .insert(attr(n, "name")?, CommentId::new(attr(n, "comment")?));
                }
                "region" => {
                    s.regions
                        .insert(attr(n, "id")?, RevisionId::new(attr(n, "revision")?));
                }
                "footnote" => {
                    let id = FootnoteId(number(n, "id")?);
                    s.footnotes.push(id);
                    if n.get("orphan") == Some("true") {
                        s.orphans.insert(id);
                    }
                }
                other => return Err(bad(format!("unknown sidecar entry {other}"))),
            }
        }
        Ok(s)
    }
}

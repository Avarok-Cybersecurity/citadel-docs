//! Conformance of a ProseMirror JSON tree to the schema: known types, declared attributes,
//! admitted children and marks. The round-trip tests run it on every encoded document, which
//! is what makes "the schema is generated from the model" a checked claim rather than a hope.

use super::SchemaSpec;
use crate::json::{child_path, PmError, PmNode};
use crate::names::node;

/// Check `root` against `spec`, reporting the first violation with its path.
pub fn check(root: &PmNode, spec: &SchemaSpec) -> Result<(), PmError> {
    if root.kind != spec.top_node {
        return Err(PmError::new(
            &root.kind,
            format!("top node must be {:?}", spec.top_node),
        ));
    }
    check_node(root, spec, &root.kind)
}

fn check_node(n: &PmNode, spec: &SchemaSpec, path: &str) -> Result<(), PmError> {
    let node_spec = spec
        .node(&n.kind)
        .ok_or_else(|| PmError::new(path, format!("unknown node type {:?}", n.kind)))?;
    for key in n.attrs.keys() {
        if !node_spec.attrs.contains(key) {
            return Err(PmError::new(
                path,
                format!("attribute {key:?} is not declared on {:?}", n.kind),
            ));
        }
    }
    if (n.kind == node::TEXT) != n.text.is_some() {
        return Err(PmError::new(path, "only text nodes carry text"));
    }
    if node_spec.children.is_empty() && !n.content.is_empty() {
        return Err(PmError::new(path, format!("{:?} is a leaf", n.kind)));
    }
    for m in &n.marks {
        let allowed = match node_spec.marks.as_str() {
            "_" => true,
            "" => false,
            list => list.split(' ').any(|name| name == m.kind),
        };
        if !allowed {
            return Err(PmError::new(
                path,
                format!("mark {:?} is not allowed on {:?}", m.kind, n.kind),
            ));
        }
        let mark_spec = spec
            .mark(&m.kind)
            .ok_or_else(|| PmError::new(path, format!("unknown mark type {:?}", m.kind)))?;
        for key in m.attrs.keys() {
            if !mark_spec.attrs.contains(key) {
                return Err(PmError::new(
                    path,
                    format!("attribute {key:?} is not declared on mark {:?}", m.kind),
                ));
            }
        }
    }
    for (i, child) in n.content.iter().enumerate() {
        let child_path = child_path(path, &child.kind, i);
        if !node_spec.children.contains(&child.kind) {
            return Err(PmError::new(
                &child_path,
                format!("{:?} is not allowed inside {:?}", child.kind, n.kind),
            ));
        }
        check_node(child, spec, &child_path)?;
    }
    Ok(())
}

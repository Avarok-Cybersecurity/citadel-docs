//! Undoing a document attribute without undoing anyone else's edit of it. The list-valued
//! attributes (comments, styles, numbering) are maps by id in the CRDT, so an undo puts back
//! only the entries its own step changed, against the list as it is now: entries the step
//! added are removed, entries it removed or changed get their old value, and entries a peer
//! added or changed meanwhile stay. Single values (metadata, settings, style defaults) go back
//! whole, as a later write of a register does.

use cdoc_crdt::DocAttr;
use cdoc_model::{Document, Numbering, StyleSheet};

/// `now` with the entries that went from `before` to `after` put back.
fn list<T: Clone + PartialEq, K: PartialEq>(
    now: &[T],
    before: &[T],
    after: &[T],
    key: impl Fn(&T) -> K,
) -> Vec<T> {
    let find = |l: &[T], k: &K| l.iter().find(|e| key(e) == *k).cloned();
    let mut out: Vec<T> = now
        .iter()
        .filter(|e| {
            let k = key(e);
            find(before, &k).is_some() || find(after, &k).is_none()
        })
        .cloned()
        .collect();
    for old in before {
        let k = key(old);
        if find(after, &k).as_ref() == Some(old) {
            continue;
        }
        match out.iter_mut().find(|e| key(e) == k) {
            Some(e) => *e = old.clone(),
            None => out.push(old.clone()),
        }
    }
    out
}

/// What undoing a step that wrote `after` over `before` writes, given the document now.
pub(crate) fn revert(model: &Document, before: &DocAttr, after: &DocAttr) -> DocAttr {
    match (before, after) {
        (DocAttr::Comments(b), DocAttr::Comments(a)) => {
            DocAttr::Comments(list(&model.comments, b, a, |c| c.id.clone()))
        }
        (DocAttr::Styles(b), DocAttr::Styles(a)) => DocAttr::Styles(StyleSheet {
            defaults: b.defaults.clone(),
            styles: list(&model.styles.styles, &b.styles, &a.styles, |s| s.id.clone()),
        }),
        (DocAttr::Numbering(b), DocAttr::Numbering(a)) => {
            let now = &model.numbering;
            DocAttr::Numbering(Numbering {
                definitions: list(&now.definitions, &b.definitions, &a.definitions, |d| d.id),
                instances: list(&now.instances, &b.instances, &a.instances, |i| i.id),
            })
        }
        _ => before.clone(),
    }
}

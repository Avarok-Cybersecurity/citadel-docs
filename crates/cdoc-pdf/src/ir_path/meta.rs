// The IR→PDF path exactly as main had it, kept only until the display-list path is proven
// to write the same bytes; removed in the commit after.
//! What the file says about itself: the information dictionary (title, producer, dates from
//! the clock port), the outline from the document's headings, and which link targets are
//! honoured.

use cdoc_layout::ir::to_pt;
use cdoc_layout::{Layout, OutlineEntry};
use pdf_writer::{Date, Finish, Pdf, Ref, TextStr};

pub use cdoc_paint::allowed_link;

/// Writes the information dictionary.
pub fn info(pdf: &mut Pdf, id: Ref, layout: &Layout, now: Option<Date>) {
    let mut info = pdf.document_info(id);
    if let Some(title) = &layout.title {
        info.title(TextStr(title));
    }
    info.producer(TextStr("Citadel Docs"));
    if let Some(now) = now {
        info.creation_date(now);
        info.modified_date(now);
    }
    info.finish();
}

/// Writes the outline tree from the headings; returns its root, or `None` without headings.
pub fn outline(pdf: &mut Pdf, alloc: &mut Ref, layout: &Layout, pages: &[Ref]) -> Option<Ref> {
    let entries: &[OutlineEntry] = &layout.outline;
    if entries.is_empty() {
        return None;
    }
    let root = alloc.bump();
    let ids: Vec<Ref> = entries.iter().map(|_| alloc.bump()).collect();
    // Each entry's parent is the nearest earlier entry with a smaller level.
    let parent: Vec<Option<usize>> = (0..entries.len())
        .map(|i| (0..i).rev().find(|&j| entries[j].level < entries[i].level))
        .collect();
    let children = |p: Option<usize>| -> Vec<usize> {
        (0..entries.len()).filter(|&i| parent[i] == p).collect()
    };
    let top = children(None);
    pdf.outline(root)
        .first(ids[top[0]])
        .last(ids[*top.last().expect("non-empty")])
        .count(i32::try_from(entries.len()).unwrap_or(i32::MAX));
    for (i, entry) in entries.iter().enumerate() {
        let siblings = children(parent[i]);
        let at = siblings
            .iter()
            .position(|&s| s == i)
            .expect("a sibling of itself");
        let mine = children(Some(i));
        let mut item = pdf.outline_item(ids[i]);
        item.title(TextStr(&entry.title));
        item.parent(parent[i].map_or(root, |p| ids[p]));
        if at > 0 {
            item.prev(ids[siblings[at - 1]]);
        }
        if let Some(next) = siblings.get(at + 1) {
            item.next(ids[*next]);
        }
        if let (Some(first), Some(last)) = (mine.first(), mine.last()) {
            item.first(ids[*first]).last(ids[*last]);
            item.count(-i32::try_from(mine.len()).unwrap_or(i32::MAX));
        }
        let page = usize::try_from(entry.page).unwrap_or(0);
        let height = layout.pages.get(page).map_or(0, |p| p.height);
        item.dest()
            .page(pages[page])
            .xyz(0.0, to_pt(height - entry.y), None);
        item.finish();
    }
    Some(root)
}

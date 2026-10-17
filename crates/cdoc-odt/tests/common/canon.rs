//! The domain a `.odt` written by this crate can hold, as a projection on generated documents.
//! Each rule names the format fact it encodes.

use cdoc_model::{CommentId, Document, FontSize, Paragraph, Table};

/// Comment ranges open and close between inlines: a continuing range keeps its place and a
/// new one is appended, so that is the only order a list of anchors can come back in.
fn anchor_order(p: &mut Paragraph) {
    let mut active: Vec<CommentId> = Vec::new();
    for inline in &mut p.inlines {
        let mut now = std::mem::take(&mut inline.marks.comments);
        let mut seen = Vec::new();
        now.retain(|id| {
            let fresh = !seen.contains(id);
            seen.push(id.clone());
            fresh
        });
        active.retain(|id| now.contains(id));
        for id in now {
            if !active.contains(&id) {
                active.push(id);
            }
        }
        inline.marks.comments.clone_from(&active);
    }
}

/// A cell the grid has no place for (spans before it cover the rest of its row) has no table
/// cell to be written as.
fn placed_cells_only(t: &mut Table) {
    let placed: Vec<(usize, usize)> = t.placements().iter().map(|p| (p.row, p.index)).collect();
    for (r, row) in t.rows.iter_mut().enumerate() {
        let mut i = 0;
        row.cells.retain(|_| {
            i += 1;
            placed.contains(&(r, i - 1))
        });
    }
}

/// The projection.
pub fn canonical(mut doc: Document) -> Document {
    // Test default (PCND): the model requires document defaults to name a font and a size; the
    // reader fills a missing one from the built-in sheet and reports it, so generated documents
    // are given one up front.
    let run = &mut doc.styles.defaults.run;
    run.font.get_or_insert_with(|| "Calibri".to_owned());
    run.size.get_or_insert(FontSize(22));
    // Ids are keys: of two styles, footnotes or comments sharing one, the format holds the first.
    let mut seen = Vec::new();
    doc.styles.styles.retain(|s| {
        !seen.contains(&s.id) && {
            seen.push(s.id.clone());
            true
        }
    });
    let mut seen = Vec::new();
    doc.footnotes.retain(|f| {
        !seen.contains(&f.id) && {
            seen.push(f.id);
            true
        }
    });
    let mut seen = Vec::new();
    doc.comments.retain(|c| {
        !seen.contains(&c.id) && {
            seen.push(c.id.clone());
            true
        }
    });
    let mut seen = Vec::new();
    doc.numbering.definitions.retain(|d| {
        !seen.contains(&d.id) && {
            seen.push(d.id);
            true
        }
    });
    let mut seen = Vec::new();
    doc.numbering.instances.retain(|i| {
        !seen.contains(&i.id) && {
            seen.push(i.id);
            true
        }
    });
    doc.for_each_paragraph_mut(&mut anchor_order);
    doc.for_each_table_mut(&mut placed_cells_only);
    let _ = cdoc_pkg::repair_references(&mut doc);
    doc.normalize();
    doc
}

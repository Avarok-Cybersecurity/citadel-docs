//! Writing where signature fields landed back into the model (`SignatureField.anchor`), so the
//! signing track knows each field's page rectangle without laying the document out again.

use crate::ir::Layout;
use cdoc_model::{Block, Document, InlineContent, Length, PageAnchor};

fn visit(blocks: &mut [Block], f: &mut dyn FnMut(&mut InlineContent)) {
    for block in blocks {
        match block {
            Block::Paragraph(p) => p.inlines.iter_mut().for_each(|i| f(&mut i.content)),
            Block::Table(t) => {
                for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    visit(&mut cell.blocks, f);
                }
            }
        }
    }
}

/// Sets `anchor` on every signature field the layout placed, and clears it on any it did not
/// (a field in a header lands on every page; the first placement is the anchor).
pub fn write_anchors(doc: &mut Document, layout: &Layout) {
    let mut apply = |content: &mut InlineContent| {
        if let InlineContent::SignatureField(field) = content {
            field.anchor = layout
                .signatures
                .iter()
                .find(|s| s.id == field.id)
                .map(|s| PageAnchor {
                    page: s.page,
                    x: Length(s.rect.x),
                    y: Length(s.rect.y),
                    width: Length(s.rect.width),
                    height: Length(s.rect.height),
                });
        }
    };
    for section in &mut doc.sections {
        visit(&mut section.blocks, &mut apply);
        let props = &mut section.properties;
        for set in [&mut props.headers, &mut props.footers] {
            for hf in [&mut set.default, &mut set.first, &mut set.even]
                .into_iter()
                .flatten()
            {
                visit(&mut hf.blocks, &mut apply);
            }
        }
    }
    for note in &mut doc.footnotes {
        visit(&mut note.blocks, &mut apply);
    }
}

//! Edits as an editor makes them, each with the `Dirty` it would report.

use super::{block, clean_blocks};
use cdoc_layout::{BlockEdit, Dirty};
use cdoc_model::{Block, Document, InlineContent, Paragraph, StyleId};
use proptest::prelude::*;

/// One edit. Positions are reduced modulo what the document has when it is applied.
#[derive(Debug, Clone)]
pub enum Op {
    /// A character typed at the end of a paragraph's first text run.
    Type(usize, usize, char),
    /// A run property changed: bold toggled and a size set on the first text run.
    Restyle(usize, usize, u16),
    /// A block inserted.
    Insert(usize, usize, Block),
    /// A block deleted.
    Delete(usize, usize),
    /// A block replaced.
    Replace(usize, usize, Block),
    /// Two blocks replaced in one relayout.
    ReplaceTwo(usize, usize, usize, Block),
    /// The default font size changed.
    Styles(u16),
    /// A section's top margin changed.
    PageSetup(usize, i32),
    /// A section's title-page setting toggled.
    Furniture(usize),
    /// A footnote's text extended.
    Footnote(char),
    /// A paragraph given a style the sheet does not have: the layout fails until it goes.
    Break(usize, usize),
}

/// An edit.
pub fn op() -> BoxedStrategy<Op> {
    let ch = prop::sample::select(vec!['a', 'W', ' ', 'é', '.', '\t']);
    prop_oneof![
        8 => (any::<usize>(), any::<usize>(), ch.clone()).prop_map(|(s, i, c)| Op::Type(s, i, c)),
        3 => (any::<usize>(), any::<usize>(), 8u16..60).prop_map(|(s, i, z)| Op::Restyle(s, i, z)),
        3 => (any::<usize>(), any::<usize>(), block()).prop_map(|(s, i, b)| Op::Insert(s, i, b)),
        2 => (any::<usize>(), any::<usize>()).prop_map(|(s, i)| Op::Delete(s, i)),
        2 => (any::<usize>(), any::<usize>(), block()).prop_map(|(s, i, b)| Op::Replace(s, i, b)),
        1 => (any::<usize>(), any::<usize>(), any::<usize>(), block())
            .prop_map(|(s, a, b, k)| Op::ReplaceTwo(s, a, b, k)),
        1 => (8u16..40).prop_map(Op::Styles),
        1 => (any::<usize>(), 0i32..3000).prop_map(|(s, t)| Op::PageSetup(s, t)),
        1 => any::<usize>().prop_map(Op::Furniture),
        1 => ch.prop_map(Op::Footnote),
        1 => (any::<usize>(), any::<usize>()).prop_map(|(s, i)| Op::Break(s, i)),
    ]
    .boxed()
}

fn first_paragraph(block: &mut Block) -> Option<&mut Paragraph> {
    match block {
        Block::Paragraph(p) => Some(p),
        Block::Table(t) => t
            .rows
            .iter_mut()
            .flat_map(|r| r.cells.iter_mut())
            .flat_map(|c| c.blocks.iter_mut())
            .find_map(first_paragraph),
    }
}

/// The first text run of the block's first paragraph (in a table, its first cell's), made
/// when there is none.
fn text_run(block: &mut Block) -> Option<&mut cdoc_model::TextRun> {
    let p = first_paragraph(block)?;
    if !p
        .inlines
        .iter()
        .any(|i| matches!(i.content, InlineContent::Text(_)))
    {
        p.inlines.push(cdoc_model::Inline::text(
            "x",
            cdoc_model::RunProperties::none(),
        ));
    }
    p.inlines.iter_mut().find_map(|i| match &mut i.content {
        InlineContent::Text(run) => Some(run),
        _ => None,
    })
}

/// `section` and `index` reduced to a block that exists, if the section has any.
fn pick(doc: &Document, section: usize, index: usize) -> Option<(usize, usize)> {
    let section = section % doc.sections.len();
    let count = doc.sections[section].blocks.len();
    (count > 0).then(|| (section, index % count))
}

fn cleaned(doc: &Document, block: &Block) -> Block {
    let mut blocks = vec![block.clone()];
    clean_blocks(doc, &mut blocks);
    blocks.remove(0)
}

/// An edit inside one existing block.
fn in_block(doc: &mut Document, op: &Op) -> Option<Dirty> {
    let (Op::Type(section, index, _) | Op::Restyle(section, index, _) | Op::Break(section, index)) =
        op
    else {
        return None;
    };
    let Some((section, index)) = pick(doc, *section, *index) else {
        return Some(Dirty::edits(Vec::new()));
    };
    let block = &mut doc.sections[section].blocks[index];
    match op {
        Op::Break(..) => {
            let paragraph = first_paragraph(block)?;
            paragraph.properties.style = Some(StyleId::new("NoSuchStyle"));
        }
        Op::Restyle(_, _, size) => {
            let run = text_run(block)?;
            run.properties.bold = Some(!run.properties.bold.unwrap_or(false));
            run.properties.size = Some(cdoc_model::FontSize(*size));
        }
        Op::Type(_, _, typed) => text_run(block)?.text.push(*typed),
        _ => return None,
    }
    Some(Dirty::edits(vec![BlockEdit::changed(section, index)]))
}

/// Blocks inserted, removed or replaced.
fn of_blocks(doc: &mut Document, op: &Op) -> Option<Dirty> {
    match op {
        Op::Insert(section, at, block) => {
            let section = section % doc.sections.len();
            let at = at % (doc.sections[section].blocks.len() + 1);
            let block = cleaned(doc, block);
            doc.sections[section].blocks.insert(at, block);
            Some(Dirty::edits(vec![BlockEdit::inserted(section, at, 1)]))
        }
        Op::Delete(section, index) => {
            let (section, index) = pick(doc, *section, *index)?;
            doc.sections[section].blocks.remove(index);
            Some(Dirty::edits(vec![BlockEdit::removed(section, index, 1)]))
        }
        Op::Replace(section, index, block) | Op::ReplaceTwo(section, index, _, block) => {
            let (section, index) = pick(doc, *section, *index)?;
            let block = cleaned(doc, block);
            let mut edits = vec![BlockEdit::changed(section, index)];
            let blocks = &mut doc.sections[section].blocks;
            if let Op::ReplaceTwo(_, _, other, _) = op {
                let other = other % blocks.len();
                if other > index {
                    blocks[other] = block.clone();
                    edits.push(BlockEdit::changed(section, other));
                }
            }
            blocks[index] = block;
            Some(Dirty::edits(edits))
        }
        _ => None,
    }
}

/// Applies `op`, returning what an editor would report as dirty.
pub fn apply(doc: &mut Document, op: &Op) -> Dirty {
    if let Some(dirty) = in_block(doc, op).or_else(|| of_blocks(doc, op)) {
        return dirty;
    }
    let sections = doc.sections.len();
    let none = Dirty::edits(Vec::new());
    match op {
        Op::Styles(size) => {
            doc.styles.defaults.run.size = Some(cdoc_model::FontSize(*size));
            Dirty {
                styles_changed: true,
                ..none
            }
        }
        Op::PageSetup(section, top) => {
            let margins = &mut doc.sections[section % sections].properties.page.margins;
            margins.top = cdoc_model::Length(*top);
            Dirty {
                page_setup_changed: true,
                ..none
            }
        }
        Op::Furniture(section) => {
            let props = &mut doc.sections[section % sections].properties;
            props.title_page = !props.title_page;
            Dirty {
                furniture_changed: true,
                ..none
            }
        }
        Op::Footnote(typed) => {
            let footnote = doc.footnotes.first_mut().and_then(|n| n.blocks.first_mut());
            if let Some(run) = footnote.and_then(text_run) {
                run.text.push(*typed);
            }
            Dirty {
                footnotes_changed: true,
                ..none
            }
        }
        _ => none,
    }
}

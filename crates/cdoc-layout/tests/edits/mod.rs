//! Documents the layout can take, built from the model's own strategies, and edits on them as
//! an editor makes them, each with the `Dirty` an editor would report.

#![allow(dead_code, reason = "each test binary uses a different part")]

mod ops;

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::test_support::strategies::{footnote, numbering, paragraph, section, table};
use cdoc_model::visit_mut::paragraphs_in;
use cdoc_model::{Block, Document, InlineContent, NumId, Paragraph, StyleId, StyleKind};
pub use ops::{apply, op, Op};
use proptest::prelude::*;

/// A block: mostly paragraphs, sometimes a table.
pub fn block() -> BoxedStrategy<Block> {
    prop_oneof![
        4 => paragraph().prop_map(Block::Paragraph),
        1 => table().prop_map(Block::Table),
    ]
    .boxed()
}

/// What a generated reference may point at: the sheet's styles and the lists that exist.
struct Known {
    paragraphs: Vec<StyleId>,
    characters: Vec<StyleId>,
    lists: Vec<NumId>,
}

fn styles(doc: &Document, kind: StyleKind) -> Vec<StyleId> {
    doc.styles
        .styles
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| s.id.clone())
        .collect()
}

fn known(doc: &Document) -> Known {
    let n = &doc.numbering;
    Known {
        paragraphs: styles(doc, StyleKind::Paragraph),
        characters: styles(doc, StyleKind::Character),
        lists: n
            .instances
            .iter()
            .filter(|i| n.definitions.iter().any(|d| d.id == i.abstract_id))
            .map(|i| i.id)
            .collect(),
    }
}

/// A generated style id mapped onto a style the sheet has (or none), deterministically.
fn remap(id: &mut Option<StyleId>, known: &[StyleId]) {
    if let Some(s) = id {
        *id = (s.0.len() % 3 != 0 && !known.is_empty())
            .then(|| known[s.0.len() % known.len()].clone());
    }
}

/// Style and list references pointed at ones that exist, so the cascade resolves and list
/// items count on real lists (whose labels then depend on every item before them).
fn clean_paragraph(p: &mut Paragraph, known: &Known) {
    remap(&mut p.properties.style, &known.paragraphs);
    if let (Some(r), false) = (&mut p.properties.numbering, known.lists.is_empty()) {
        let at = usize::try_from(r.num_id.0).unwrap_or(0) % known.lists.len();
        r.num_id = known.lists[at];
    }
    for inline in &mut p.inlines {
        if let InlineContent::Text(run) = &mut inline.content {
            remap(&mut run.properties.style, &known.characters);
        }
    }
}

/// References in `blocks` pointed at what `doc` has.
pub fn clean_blocks(doc: &Document, blocks: &mut [Block]) {
    let known = known(doc);
    paragraphs_in(blocks, &mut |para| clean_paragraph(para, &known));
}

fn strip_refs(blocks: &mut [Block]) {
    paragraphs_in(blocks, &mut |para| {
        para.inlines
            .retain(|i| !matches!(i.content, InlineContent::FootnoteRef { .. }));
    });
}

/// Footnote references inside footnotes and furniture tie numbering to placement and to
/// every page; most generated documents leave them out so the incremental paths get
/// exercised, some keep them so the fallbacks do.
fn clean(doc: &mut Document, notes_cite_notes: bool, furniture_cites_notes: bool) {
    if known(doc).lists.is_empty() {
        doc.numbering = kitchen_sink().numbering;
    }
    let known = known(doc);
    doc.for_each_paragraph_mut(&mut |para| clean_paragraph(para, &known));
    for level in doc
        .numbering
        .definitions
        .iter_mut()
        .flat_map(|d| &mut d.levels)
    {
        if let Some(run) = &mut level.run {
            run.style = None;
        }
    }
    if !notes_cite_notes {
        for note in &mut doc.footnotes {
            strip_refs(&mut note.blocks);
        }
    }
    if !furniture_cites_notes {
        for section in &mut doc.sections {
            let p = &mut section.properties;
            for set in [&mut p.headers, &mut p.footers] {
                for hf in [&mut set.default, &mut set.first, &mut set.even]
                    .into_iter()
                    .flatten()
                {
                    strip_refs(&mut hf.blocks);
                }
            }
        }
    }
}

/// A document of one to three sections of 2 to 24 blocks, with lists, notes and furniture.
pub fn document() -> BoxedStrategy<Document> {
    let section = (section(), prop::collection::vec(block(), 2..=24)).prop_map(|(mut s, b)| {
        s.blocks = b;
        s
    });
    (
        numbering(),
        prop::collection::vec(section, 1..=3),
        prop::collection::vec(footnote(), 0..=3),
        any::<bool>(),
        prop::bool::weighted(0.15),
        prop::bool::weighted(0.15),
    )
        .prop_map(
            |(numbering, sections, footnotes, even_odd, notes, furniture)| {
                let mut doc = Document::blank();
                doc.numbering = numbering;
                doc.sections = sections;
                doc.footnotes = footnotes;
                doc.settings.even_and_odd_headers = even_odd;
                clean(&mut doc, notes, furniture);
                doc
            },
        )
        .boxed()
}

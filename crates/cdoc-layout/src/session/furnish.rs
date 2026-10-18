//! Headers, footers and body fields, set again only on the pages that need it: a page whose
//! body was set anew, or whose header, footer or fields would print something different (its
//! number or the page count changed and something on it prints that).

use super::record::{FurnitureKey, PageMeta};
use super::splice::Slot;
use crate::context::Ctx;
use crate::flow::Sheet;
use crate::fragment::FieldRun;
use crate::furniture::{chosen, furnish_page, page_numbers};
use crate::ir::Page;
use crate::LayoutError;
use cdoc_model::{Block, Document, FieldKind, HeaderFooter, InlineContent};

/// The pages, what each was, and the indices of those furnished.
pub type Furnished = (Vec<Page>, Vec<PageMeta>, Vec<usize>);

fn prints(hf: Option<&HeaderFooter>, kind: &FieldKind) -> bool {
    hf.is_some_and(|hf| {
        hf.blocks
            .iter()
            .flat_map(Block::paragraphs)
            .flat_map(|p| &p.inlines)
            .any(|i| matches!(&i.content, InlineContent::Field(f) if f.field == *kind))
    })
}

/// Whether any header or footer references a footnote: its number depends on every page
/// before it, so every page is furnished again.
pub fn furniture_numbers_notes(doc: &Document) -> bool {
    doc.sections.iter().any(|s| {
        let p = &s.properties;
        p.headers
            .present()
            .into_iter()
            .chain(p.footers.present())
            .flat_map(|(_, hf)| hf.blocks.iter().flat_map(Block::paragraphs))
            .flat_map(|p| &p.inlines)
            .any(|i| matches!(i.content, InlineContent::FootnoteRef { .. }))
    })
}

fn key(
    doc: &Document,
    section: usize,
    first_of_section: bool,
    number: u32,
    count: usize,
    fields: &[FieldRun],
) -> FurnitureKey {
    let props = &doc.sections[section].properties;
    let (header, footer) = chosen(doc, section, first_of_section, number);
    let uses = |kind: FieldKind| {
        fields.iter().any(|f| f.kind == kind) || prints(header, &kind) || prints(footer, &kind)
    };
    FurnitureKey {
        section,
        first: first_of_section && props.title_page,
        even: doc.settings.even_and_odd_headers && number.is_multiple_of(2),
        number: uses(FieldKind::PageNumber).then(|| number.to_string()),
        count: uses(FieldKind::PageCount).then_some(count),
    }
}

fn furnish(
    ctx: &mut Ctx<'_, '_>,
    mut sheet: Sheet,
    number: u32,
    count: usize,
    key: FurnitureKey,
) -> Result<(Page, PageMeta), LayoutError> {
    let fields = sheet.fields.clone();
    let page = &sheet.page;
    let body = [page.draws.len(), page.links.len(), page.lines.len()];
    furnish_page(ctx, &mut sheet, number, count)?;
    let meta = PageMeta {
        body_draws: body[0],
        body_links: body[1],
        body_lines: body[2],
        fields,
        section: sheet.section,
        first_of_section: sheet.first_of_section,
        key,
        trace: ctx.take_trace(),
    };
    Ok((sheet.page, meta))
}

/// The pages furnished, and which of them were (or changed body). `force` furnishes all.
pub fn furnish_all(
    ctx: &mut Ctx<'_, '_>,
    slots: Vec<Slot>,
    force: bool,
) -> Result<Furnished, LayoutError> {
    let doc = ctx.doc;
    let where_ = |slot: &Slot| match slot {
        Slot::Stored(_, m) => (m.section, m.first_of_section),
        Slot::Body(s) => (s.section, s.first_of_section),
    };
    let numbers = page_numbers(doc, slots.iter().map(where_));
    let count = slots.len();
    let mut pages = Vec::with_capacity(count);
    let mut metas = Vec::with_capacity(count);
    let mut changed = Vec::new();
    for (index, (slot, number)) in slots.into_iter().zip(numbers).enumerate() {
        let (section, first) = where_(&slot);
        let (page, meta) = match slot {
            Slot::Stored(page, meta) => {
                let key = key(doc, section, first, number, count, &meta.fields);
                if !force && meta.key == key {
                    pages.push(page);
                    metas.push(meta);
                    continue;
                }
                let mut page = page;
                page.draws.truncate(meta.body_draws);
                page.links.truncate(meta.body_links);
                page.lines.truncate(meta.body_lines);
                let sheet = Sheet {
                    page,
                    section,
                    first_of_section: first,
                    fields: meta.fields,
                };
                furnish(ctx, sheet, number, count, key)?
            }
            Slot::Body(sheet) => {
                let key = key(doc, section, first, number, count, &sheet.fields);
                furnish(ctx, sheet, number, count, key)?
            }
        };
        pages.push(page);
        metas.push(meta);
        changed.push(index);
    }
    Ok((pages, metas, changed))
}

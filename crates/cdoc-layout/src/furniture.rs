//! What every page carries besides its body, set once the body is paginated and the page count
//! is known: headers and footers (first, even or default, as the section and document settings
//! choose) and the page-dependent fields in the body.

use crate::context::{Ctx, FieldValues};
use crate::flow::Sheet;
use crate::fragment::{Fragment, Mark};
use crate::geometry::Geometry;
use crate::ir::{Draw, LinkArea, TextRun};
use crate::labels::{format_number, Counters};
use crate::stack::stack;
use crate::LayoutError;
use cdoc_model::{FieldKind, HeaderFooter, HeaderFooterSet, NumberFormat};

/// Printed page numbers: restarting where a section says so, continuing otherwise.
fn page_numbers(ctx: &Ctx<'_, '_>, sheets: &[Sheet]) -> Vec<u32> {
    let mut out = Vec::with_capacity(sheets.len());
    let mut next = 1;
    for sheet in sheets {
        if sheet.first_of_section {
            if let Some(start) = ctx.doc.sections[sheet.section]
                .properties
                .page
                .page_numbering
                .start
            {
                next = start;
            }
        }
        out.push(next);
        next += 1;
    }
    out
}

fn choose(set: &HeaderFooterSet, first: bool, even: bool) -> Option<&HeaderFooter> {
    if first {
        set.first.as_ref()
    } else if even {
        set.even.as_ref()
    } else {
        set.default.as_ref()
    }
}

fn place(sheet: &mut Sheet, strip: Fragment, x: i32, y: i32) {
    let mut holder = Fragment::empty(0);
    holder.absorb(strip, x, y);
    sheet.page.draws.extend(holder.draws);
    for mark in holder.marks {
        if let Mark::Link { rect, href } = mark {
            sheet.page.links.push(LinkArea { rect, href });
        }
    }
}

/// Sets headers, footers and body fields on every sheet.
pub fn furnish(ctx: &mut Ctx<'_, '_>, sheets: &mut [Sheet]) -> Result<(), LayoutError> {
    let doc = ctx.doc;
    let numbers = page_numbers(ctx, sheets);
    let pages = sheets.len().to_string();
    for (sheet, number) in sheets.iter_mut().zip(numbers) {
        let section = &doc.sections[sheet.section].properties;
        let geometry = Geometry::of(&section.page);
        let format = match section.page.page_numbering.format {
            NumberFormat::Bullet | NumberFormat::None => NumberFormat::Decimal,
            other => other,
        };
        let values = FieldValues {
            page: format_number(number, format),
            pages: pages.clone(),
        };
        let first = sheet.first_of_section && section.title_page;
        let even = doc.settings.even_and_odd_headers && number % 2 == 0;
        let header = choose(&section.headers, first, even);
        let footer = choose(&section.footers, first, even);
        ctx.fields = Some(values.clone());
        let mut counters = Counters::new(&doc.numbering);
        if let Some(header) = header {
            let strip = stack(ctx, &header.blocks, geometry.width, &mut counters)?;
            place(sheet, strip, geometry.left, geometry.header_top);
        }
        if let Some(footer) = footer {
            let strip = stack(ctx, &footer.blocks, geometry.width, &mut counters)?;
            let top = geometry.footer_bottom - strip.height;
            place(sheet, strip, geometry.left, top);
        }
        ctx.fields = None;
        for (kind, run) in std::mem::take(&mut sheet.fields) {
            let value = match kind {
                FieldKind::PageNumber => values.page.clone(),
                FieldKind::PageCount => values.pages.clone(),
                FieldKind::Date { .. } => run.text.clone(),
            };
            let text = ctx.drawable(&value, run.face);
            sheet.page.draws.push(Draw::Text(TextRun { text, ..run }));
        }
    }
    Ok(())
}

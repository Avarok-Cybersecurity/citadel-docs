//! What every page carries besides its body, set once the body is paginated and the page count
//! is known: headers and footers (first, even or default, as the section and document settings
//! choose) and the page-dependent fields in the body.

use crate::context::{Ctx, FieldValues};
use crate::flow::Sheet;
use crate::fragment::{Fragment, Mark};
use crate::geometry::Geometry;
use crate::ir::{Draw, GlyphRun, LinkArea, Region};
use crate::labels::{format_number, Counters};
use crate::stack::stack;
use crate::LayoutError;
use cdoc_model::{FieldKind, HeaderFooter, HeaderFooterKind, HeaderFooterSet, NumberFormat};

/// Printed page numbers for sheets given as `(section, first_of_section)`: restarting where a
/// section says so, continuing otherwise.
pub fn page_numbers(
    doc: &cdoc_model::Document,
    sheets: impl Iterator<Item = (usize, bool)>,
) -> Vec<u32> {
    let mut out = Vec::new();
    let mut next = 1;
    for (section, first_of_section) in sheets {
        if first_of_section {
            if let Some(start) = doc.sections[section].properties.page.page_numbering.start {
                next = start;
            }
        }
        out.push(next);
        next += 1;
    }
    out
}

/// Which of its section's headers and footers a sheet shows.
#[must_use]
pub fn shown(
    doc: &cdoc_model::Document,
    section: usize,
    first_of_section: bool,
    number: u32,
) -> HeaderFooterKind {
    if first_of_section && doc.sections[section].properties.title_page {
        HeaderFooterKind::First
    } else if doc.settings.even_and_odd_headers && number.is_multiple_of(2) {
        HeaderFooterKind::Even
    } else {
        HeaderFooterKind::Default
    }
}

/// The header and footer a sheet shows.
pub fn chosen(
    doc: &cdoc_model::Document,
    section: usize,
    first_of_section: bool,
    number: u32,
) -> (Option<&HeaderFooter>, Option<&HeaderFooter>) {
    let props = &doc.sections[section].properties;
    let kind = shown(doc, section, first_of_section, number);
    (get(&props.headers, kind), get(&props.footers, kind))
}

const fn get(set: &HeaderFooterSet, kind: HeaderFooterKind) -> Option<&HeaderFooter> {
    match kind {
        HeaderFooterKind::First => set.first.as_ref(),
        HeaderFooterKind::Even => set.even.as_ref(),
        HeaderFooterKind::Default => set.default.as_ref(),
    }
}

fn place(sheet: &mut Sheet, mut strip: Fragment, x: i32, y: i32, region: Region) {
    for mark in std::mem::take(&mut strip.marks) {
        match mark.shifted(x, y) {
            Mark::Link { rect, href } => sheet.page.links.push(LinkArea { rect, href }),
            Mark::Line(line) => sheet.page.lines.push(line.placed(region, &[])),
            _ => {}
        }
    }
    sheet
        .page
        .draws
        .extend(strip.draws.into_iter().map(|d| d.shifted(x, y)));
}

/// Sets the header, footer and body fields of one sheet, printed as page `number` of `pages`.
pub fn furnish_page(
    ctx: &mut Ctx<'_, '_>,
    sheet: &mut Sheet,
    number: u32,
    pages: usize,
) -> Result<(), LayoutError> {
    let doc = ctx.doc;
    let section = &doc.sections[sheet.section].properties;
    let geometry = Geometry::of(&section.page);
    let format = match section.page.page_numbering.format {
        NumberFormat::Bullet | NumberFormat::None => NumberFormat::Decimal,
        other => other,
    };
    let values = FieldValues {
        page: format_number(number, format),
        pages: pages.to_string(),
    };
    let (header, footer) = chosen(doc, sheet.section, sheet.first_of_section, number);
    let kind = shown(doc, sheet.section, sheet.first_of_section, number);
    let at = u32::try_from(sheet.section).unwrap_or(u32::MAX);
    ctx.fields = Some(values.clone());
    let mut counters = Counters::new(&doc.numbering);
    if let Some(header) = header {
        let strip = stack(ctx, &header.blocks, geometry.width, &mut counters)?;
        let region = Region::Header { section: at, kind };
        place(sheet, strip, geometry.left, geometry.header_top, region);
    }
    if let Some(footer) = footer {
        let strip = stack(ctx, &footer.blocks, geometry.width, &mut counters)?;
        let top = geometry.footer_bottom - strip.height;
        let region = Region::Footer { section: at, kind };
        place(sheet, strip, geometry.left, top, region);
    }
    ctx.fields = None;
    for field in std::mem::take(&mut sheet.fields) {
        let run = field.run;
        let value = match field.kind {
            FieldKind::PageNumber => values.page.clone(),
            FieldKind::PageCount => values.pages.clone(),
            FieldKind::Date { .. } => run.text.clone(),
        };
        let text = ctx.drawable(&value, run.face, |_| {});
        let mut glyphs = Vec::new();
        ctx.fonts
            .shape(run.face, run.size, &text, field.features, 0, &mut glyphs);
        sheet.page.draws.push(Draw::Text(GlyphRun {
            text,
            glyphs,
            ..run
        }));
    }
    Ok(())
}

//! What the file says about itself, from the display list's document table: the information
//! dictionary (title, producer, dates from the clock port), the outline from the document's
//! headings, and which link targets are honoured.

use crate::PdfError;
use cdoc_layout::ir::to_pt;
use cdoc_layout::Pt100;
use cdoc_paint::DocumentInfo;
use pdf_writer::{Date, Finish, Pdf, Ref, TextStr};

pub use cdoc_paint::allowed_link;

fn number(text: &str, range: std::ops::Range<usize>) -> Option<u16> {
    text.get(range)?.parse().ok()
}

/// An RFC 3339 timestamp (`2026-10-04T12:00:00Z`, `…+02:00`) as a PDF date, or `None` when it
/// is not one.
#[must_use]
pub fn pdf_date(rfc3339: &str) -> Option<Date> {
    let t = rfc3339.trim();
    let sep = |i: usize, c: &[u8]| t.as_bytes().get(i).is_some_and(|b| c.contains(b));
    if !(sep(4, b"-") && sep(7, b"-") && sep(10, b"Tt ") && sep(13, b":") && sep(16, b":")) {
        return None;
    }
    let year = number(t, 0..4)?;
    let mut date = Date::new(year)
        .month(
            u8::try_from(number(t, 5..7)?)
                .ok()
                .filter(|m| (1..=12).contains(m))?,
        )
        .day(
            u8::try_from(number(t, 8..10)?)
                .ok()
                .filter(|d| (1..=31).contains(d))?,
        )
        .hour(u8::try_from(number(t, 11..13)?).ok().filter(|h| *h < 24)?)
        .minute(u8::try_from(number(t, 14..16)?).ok().filter(|m| *m < 60)?)
        .second(u8::try_from(number(t, 17..19)?).ok().filter(|s| *s < 61)?);
    let zone = t
        .get(19..)?
        .trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if zone.eq_ignore_ascii_case("z") {
        date = date.utc_offset_hour(0).utc_offset_minute(0);
    } else {
        let sign: i8 = match zone.as_bytes().first()? {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let hours = i8::try_from(number(zone, 1..3)?).ok()?;
        let minutes = u8::try_from(number(zone, 4..6)?).ok()?;
        date = date
            .utc_offset_hour(sign * hours)
            .utc_offset_minute(minutes);
    }
    Some(date)
}

/// Writes the information dictionary.
pub fn info(pdf: &mut Pdf, id: Ref, doc: &DocumentInfo, now: Option<Date>) {
    let mut info = pdf.document_info(id);
    if let Some(title) = &doc.title {
        info.title(TextStr(title));
    }
    info.producer(TextStr("Citadel Docs"));
    if let Some(now) = now {
        info.creation_date(now);
        info.modified_date(now);
    }
    info.finish();
}

/// Writes the outline tree from the headings, `pages` being each page's object and height;
/// returns its root, or `None` without headings. A heading on a page the list lacks is an
/// error.
pub fn outline(
    pdf: &mut Pdf,
    alloc: &mut Ref,
    doc: &DocumentInfo,
    pages: &[(Ref, Pt100)],
) -> Result<Option<Ref>, PdfError> {
    let entries = &doc.outline;
    if let Some(lost) = entries
        .iter()
        .find(|e| usize::try_from(e.page).map_or(true, |p| p >= pages.len()))
    {
        return Err(PdfError::NoOutlinePage(lost.page));
    }
    if entries.is_empty() {
        return Ok(None);
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
        let (page, height) = pages[usize::try_from(entry.page).unwrap_or(usize::MAX)];
        item.dest()
            .page(page)
            .xyz(0.0, to_pt(height - entry.y), None);
        item.finish();
    }
    Ok(Some(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_becomes_a_pdf_date_and_nonsense_does_not() {
        assert!(pdf_date("2026-10-04T12:00:00Z").is_some());
        assert!(pdf_date("2026-10-04T12:00:00.123+02:00").is_some());
        assert!(pdf_date("2026-13-04T12:00:00Z").is_none());
        assert!(pdf_date("yesterday").is_none());
        assert!(
            pdf_date("2026-10-04T12:00:00").is_none(),
            "a zone is required"
        );
    }
}

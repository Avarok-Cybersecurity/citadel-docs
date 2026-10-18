//! Matching a drawn text run against the document's units at a cursor.
//!
//! A cursor reads one **region**: the body, which skips header, footer and footnote subtrees
//! whole, or one of those subtrees ("aside"), which ends at its closing `End`.

use crate::position::Position;
use cdoc_crdt::{atom_at, Atom, AtomKind, BridgeError, Doc, Id, Unit, Units};
use cdoc_layout::Pt100;
use cdoc_model::InlineContent;
use cdoc_paint::{nominal_pen, Op};
use std::collections::BTreeMap;

/// The document units from a gap on, with the id of the unit before it.
#[derive(Clone)]
pub(super) struct Cursor<'d> {
    doc: &'d Doc,
    units: Units<'d>,
    prev: Option<Id>,
    /// Reading an aside: the tables open inside it (`None` for the body).
    aside: Option<u32>,
}

/// A run matched at the cursor.
pub(super) struct Matched<'d> {
    /// Ids of the matched characters with the run character each starts at.
    pub(super) chars: Vec<(Id, usize)>,
    /// The unit just before the first matched character.
    pub(super) before: Option<Position>,
    /// A block boundary was skipped to reach it.
    pub(super) crossed: bool,
    /// Empty paragraphs skipped on the way (their start atoms).
    pub(super) empties: Vec<Id>,
    pub(super) next: Cursor<'d>,
}

impl<'d> Cursor<'d> {
    /// The body from gap `gap` on.
    pub(super) fn body(doc: &'d Doc, gap: usize) -> Self {
        Self {
            doc,
            units: doc.units_from(gap),
            prev: gap.checked_sub(1).and_then(|i| doc.id_at(i)),
            aside: None,
        }
    }

    /// The aside whose start atom is unit `index`.
    pub(super) fn aside(doc: &'d Doc, index: usize) -> Self {
        Self {
            doc,
            units: doc.units_from(index + 1),
            prev: doc.id_at(index),
            aside: Some(0),
        }
    }

    /// The next unit to look past while searching, `None` at the region's end; skips whole
    /// asides when reading the body.
    fn advance(&mut self) -> Option<(Id, Unit)> {
        let (id, unit) = self.units.next()?;
        match (unit, &mut self.aside) {
            (Unit::Atom(AtomKind::HeaderFooter | AtomKind::Footnote), None) => {
                let mut depth = 0u32;
                for (_, u) in self.units.by_ref() {
                    match u {
                        Unit::Atom(AtomKind::Table) => depth += 1,
                        Unit::Atom(AtomKind::End) if depth == 0 => break,
                        Unit::Atom(AtomKind::End) => depth -= 1,
                        _ => {}
                    }
                }
            }
            (Unit::Atom(AtomKind::Table), Some(depth)) => *depth += 1,
            (Unit::Atom(AtomKind::End), Some(0))
            | (
                Unit::Atom(AtomKind::Section | AtomKind::HeaderFooter | AtomKind::Footnote),
                Some(_),
            ) => {
                return None;
            }
            (Unit::Atom(AtomKind::End), Some(depth)) => *depth -= 1,
            _ => {}
        }
        Some((id, unit))
    }
}

/// How many run characters doc character `c` accounts for at `run[at..]`, as the layout
/// draws it (itself, capitalised, or the missing-glyph marker).
fn drawn_as(c: char, run: &[char], at: usize) -> Option<usize> {
    let rest = &run[at..];
    if rest.first() == Some(&c) {
        return Some(1);
    }
    let upper: Vec<char> = c.to_uppercase().collect();
    if rest.starts_with(&upper) {
        return Some(upper.len());
    }
    rest.starts_with(&['[', '?', ']']).then_some(3)
}

/// Whether the inline object `id` is drawn as text that can share a run with the text around
/// it: a field's value. Images, form and signature fields are widgets (whose captions are
/// runs of their own), tabs and breaks end a run.
fn drawn_as_text(doc: &Doc, id: Id) -> Result<bool, BridgeError> {
    let Some(at) = doc.locate(id).filter(|l| l.visible) else {
        return Ok(false);
    };
    Ok(matches!(
        atom_at(doc, at.index)?,
        Some(Atom::Inline(InlineContent::Field(_)))
    ))
}

type Attempt<'d> = Option<(Vec<(Id, usize)>, Cursor<'d>)>;

fn attempt<'d>(start: &Cursor<'d>, run: &[char]) -> Result<Attempt<'d>, BridgeError> {
    let mut c = start.clone();
    let mut at = 0;
    let mut chars = Vec::new();
    while at < run.len() {
        let Some((id, unit)) = c.units.next() else {
            return Ok(None);
        };
        chars.push((id, at));
        c.prev = Some(id);
        let step = match unit {
            Unit::Char(ch) => drawn_as(ch, run, at).map(|n| at + n),
            // A field's value inside a run of matched text takes the characters up to where
            // the next character is drawn, or the rest of the run; a run of nothing but a
            // field's value is not matched.
            Unit::Atom(AtomKind::Inline) if drawn_as_text(c.doc, id)? => {
                match c.units.clone().next() {
                    Some((_, Unit::Char(next))) => {
                        (at..run.len()).find(|&j| drawn_as(next, run, j).is_some())
                    }
                    _ => (at > 0).then_some(run.len()),
                }
            }
            Unit::Atom(_) => None,
        };
        let Some(next) = step else {
            return Ok(None);
        };
        at = next;
    }
    Ok(Some((chars, c)))
}

const fn ends_block(unit: Option<Unit>) -> bool {
    match unit {
        None => true,
        Some(Unit::Atom(k)) => k.is_structural(),
        Some(Unit::Char(_)) => false,
    }
}

/// `run` matched at `start` or after the blanks and boundaries following it.
pub(super) fn matched<'d>(
    start: &Cursor<'d>,
    run: &[char],
) -> Result<Option<Matched<'d>>, BridgeError> {
    let mut c = start.clone();
    let (mut crossed, mut empties) = (false, Vec::new());
    loop {
        if let Some((chars, next)) = attempt(&c, run)? {
            return Ok(Some(Matched {
                chars,
                before: c.prev.map(Position::after),
                crossed,
                empties,
                next,
            }));
        }
        let mut ahead = c.clone();
        let Some((id, unit)) = ahead.advance() else {
            return Ok(None);
        };
        match unit {
            Unit::Char(ch) if !ch.is_whitespace() => return Ok(None),
            Unit::Char(_) | Unit::Atom(AtomKind::Inline) => {}
            Unit::Atom(kind) => {
                crossed = true;
                let next = ahead.units.clone().next().map(|(_, u)| u);
                if kind == AtomKind::Paragraph && ends_block(next) {
                    empties.push(id);
                }
            }
        }
        ahead.prev = Some(id);
        c = ahead;
    }
}

/// The empty paragraphs left in the cursor's region after its last match.
pub(super) fn trailing_empties(mut cursor: Cursor<'_>) -> Vec<Id> {
    let mut out = Vec::new();
    while let Some((id, unit)) = cursor.advance() {
        match unit {
            Unit::Atom(AtomKind::Paragraph)
                if ends_block(cursor.units.clone().next().map(|u| u.1)) =>
            {
                out.push(id);
            }
            Unit::Char(c) if !c.is_whitespace() => break,
            _ => {}
        }
    }
    out
}

/// Pen positions after each glyph of a run, from the face's advances.
pub(super) fn pens(
    op: &Op,
    advances: &BTreeMap<u16, u16>,
    upem: u16,
) -> Option<(Pt100, Vec<Pt100>)> {
    let Op::TextRun { head, glyphs } = op else {
        return None;
    };
    let mut units = 0i64;
    let mut out = vec![head.x];
    for g in glyphs {
        units += i64::from(*advances.get(g)?);
        out.push(head.x + nominal_pen(units, head.size, upem));
    }
    Some((head.y, out))
}

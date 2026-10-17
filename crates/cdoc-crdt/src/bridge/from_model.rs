//! Model to CRDT: the tree written out as the flat sequence, then formatting as the fewest
//! mark operations that reproduce it, then the document attributes.

use super::{run_props, Atom, BridgeError, DocAttr, Furniture};
use crate::codec::to_bytes;
use crate::doc::Doc;
use crate::id::ClientId;
use crate::marks::{MarkKey, RunKey};
use cdoc_model::{Block, CommentId, Document, HeaderFooterSet, InlineContent, Marks};
use std::ops::Range;

/// An inline's units and what formats it.
struct Span {
    range: Range<usize>,
    /// Character properties as `(key, value)`; `None` for an atom, which has none.
    run: Option<Vec<(RunKey, Vec<u8>)>>,
    link: Option<Vec<u8>>,
    revision: Option<Vec<u8>>,
    comments: Vec<CommentId>,
}

struct Writer {
    doc: Doc,
    spans: Vec<Span>,
}

impl Writer {
    fn atom(&mut self, atom: &Atom) -> Result<(), BridgeError> {
        let end = self.doc.len();
        Ok(self.doc.insert_atom(end, atom)?)
    }

    fn blocks(&mut self, blocks: &[Block]) -> Result<(), BridgeError> {
        for block in blocks {
            match block {
                Block::Paragraph(p) => {
                    self.atom(&Atom::Paragraph(p.properties.clone()))?;
                    for inline in &p.inlines {
                        self.inline(&inline.content, &inline.marks)?;
                    }
                }
                Block::Table(t) => {
                    self.atom(&Atom::Table {
                        properties: t.properties.clone(),
                        column_widths: t.column_widths.clone(),
                    })?;
                    for row in &t.rows {
                        self.atom(&Atom::Row {
                            height: row.height,
                            cant_split: row.cant_split,
                        })?;
                        for cell in &row.cells {
                            self.atom(&Atom::Cell {
                                properties: cell.properties.clone(),
                                header: cell.header,
                            })?;
                            self.blocks(&cell.blocks)?;
                        }
                    }
                    self.atom(&Atom::End)?;
                }
            }
        }
        Ok(())
    }

    fn inline(&mut self, content: &InlineContent, marks: &Marks) -> Result<(), BridgeError> {
        let start = self.doc.len();
        let run = match content {
            InlineContent::Text(run) => {
                self.doc.insert_text(start, &run.text)?;
                Some(run_props::decompose(&run.properties)?)
            }
            other => {
                self.atom(&Atom::Inline(other.clone()))?;
                None
            }
        };
        self.spans.push(Span {
            range: start..self.doc.len(),
            run,
            link: marks.link.as_ref().map(to_bytes).transpose()?,
            revision: marks.revision.as_ref().map(to_bytes).transpose()?,
            comments: marks.comments.clone(),
        });
        Ok(())
    }

    fn furniture(
        &mut self,
        set: &HeaderFooterSet,
        furniture: Furniture,
    ) -> Result<(), BridgeError> {
        for (kind, hf) in set.present() {
            self.atom(&Atom::HeaderFooter { furniture, kind })?;
            self.blocks(&hf.blocks)?;
            self.atom(&Atom::End)?;
        }
        Ok(())
    }

    /// One mark per maximal stretch of consecutive participating spans with one value.
    fn single(
        &mut self,
        key: &MarkKey,
        value: impl Fn(&Span) -> Option<Option<&Vec<u8>>>,
    ) -> Result<(), BridgeError> {
        let mut open: Option<(Range<usize>, Vec<u8>)> = None;
        let mut groups = Vec::new();
        for span in &self.spans {
            let Some(v) = value(span) else { continue };
            match (&mut open, v) {
                (Some((range, current)), Some(v)) if current == v => range.end = span.range.end,
                (_, v) => {
                    groups.extend(open.take());
                    open = v.map(|v| (span.range.clone(), v.clone()));
                }
            }
        }
        groups.extend(open);
        for (range, v) in groups {
            self.doc.mark_raw(range, key.clone(), Some(v))?;
        }
        Ok(())
    }

    fn marks(&mut self) -> Result<(), BridgeError> {
        for key in RunKey::ALL {
            self.single(&MarkKey::Run(key), |s| {
                let fields = s.run.as_ref()?;
                Some(fields.iter().find(|(k, _)| *k == key).map(|(_, v)| v))
            })?;
        }
        self.single(&MarkKey::Link, |s| Some(s.link.as_ref()))?;
        self.single(&MarkKey::Revision, |s| Some(s.revision.as_ref()))?;
        let mut groups: Vec<(Range<usize>, Vec<CommentId>)> = Vec::new();
        for span in &self.spans {
            match groups.last_mut() {
                Some((range, ids)) if *ids == span.comments && range.end == span.range.start => {
                    range.end = span.range.end;
                }
                _ => groups.push((span.range.clone(), span.comments.clone())),
            }
        }
        for (range, ids) in groups {
            for id in ids {
                let key = MarkKey::Comment(id.as_str().to_owned());
                self.doc.mark_raw(range.clone(), key, Some(Vec::new()))?;
            }
        }
        Ok(())
    }
}

/// A replica holding `document`, written as `client`. The inverse of
/// [`to_model`](super::to_model): `to_model(&from_model(d, c)?)` is `d` normalised.
pub fn from_model(document: &Document, client: ClientId) -> Result<Doc, BridgeError> {
    let mut w = Writer {
        doc: Doc::new(client),
        spans: Vec::new(),
    };
    for section in &document.sections {
        let mut properties = section.properties.clone();
        properties.headers = HeaderFooterSet::none();
        properties.footers = HeaderFooterSet::none();
        w.atom(&Atom::Section(properties))?;
        w.furniture(&section.properties.headers, Furniture::Header)?;
        w.furniture(&section.properties.footers, Furniture::Footer)?;
        w.blocks(&section.blocks)?;
    }
    for footnote in &document.footnotes {
        w.atom(&Atom::Footnote(footnote.id))?;
        w.blocks(&footnote.blocks)?;
        w.atom(&Atom::End)?;
    }
    w.marks()?;
    let mut doc = w.doc;
    doc.set_attr(&DocAttr::Meta(document.meta.clone()))?;
    doc.set_attr(&DocAttr::Settings(document.settings.clone()))?;
    doc.set_attr(&DocAttr::Styles(document.styles.clone()))?;
    doc.set_attr(&DocAttr::Numbering(document.numbering.clone()))?;
    doc.set_attr(&DocAttr::Comments(document.comments.clone()))?;
    Ok(doc)
}

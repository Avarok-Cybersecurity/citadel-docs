//! Caret movement, click selection, composition and the request dispatcher.

use super::{Applied, Editor};
use crate::commands::Command;
use crate::error::EditError;
use crate::ime::Composition;
use crate::position::{caret_gap, is_caret_gap, Direction, Granularity, Position, Selection};
use crate::text::{paragraph, step_cluster, word_at, word_boundary};
use crate::wire::Request;

impl Editor<'_> {
    fn at(&self, gap: usize) -> Result<Position, EditError> {
        Position::at(&self.doc, gap).ok_or(EditError::UnknownPosition)
    }

    fn last_gap(&self) -> Result<usize, EditError> {
        let end = self
            .outline
            .nodes
            .iter()
            .find(|n| n.kind == cdoc_crdt::AtomKind::Footnote)
            .and_then(|n| self.doc.locate(n.id))
            .map_or(self.doc.len(), |l| l.index);
        (1..=end)
            .rev()
            .find(|&g| is_caret_gap(&self.doc, g))
            .map_or_else(|| caret_gap(&self.doc, 0), Ok)
    }

    /// Where the caret goes from gap `h`.
    fn target(&mut self, g: Granularity, dir: Direction, h: usize) -> Result<usize, EditError> {
        let doc = &self.doc;
        let back = dir == Direction::Backward;
        Ok(match g {
            Granularity::Char => step_cluster(doc, h, dir)?,
            Granularity::Word => {
                let (_, para) = paragraph(doc, &self.outline, h)?;
                let at_edge = if back { h == para.start } else { h == para.end };
                if at_edge {
                    step_cluster(doc, h, dir)?
                } else {
                    word_boundary(doc, h, &para, dir)
                }
            }
            Granularity::LineBoundary => {
                let (first, last) = self
                    .geometry
                    .line_ends(doc, h)
                    .ok_or(EditError::UnknownPosition)?;
                (if back { first } else { last }).index(doc)?
            }
            Granularity::Line => {
                let x = match self.goal_x {
                    Some(x) => x,
                    None => self.caret_rect(self.at(h)?)?.1.x,
                };
                self.goal_x = Some(x);
                match self.geometry.vertical(&self.doc, h, back, x) {
                    Some(p) => p.index(&self.doc)?,
                    None if back => caret_gap(&self.doc, 0)?,
                    None => self.last_gap()?,
                }
            }
            Granularity::Paragraph => {
                let (_, para) = paragraph(doc, &self.outline, h)?;
                if back && h > para.start {
                    para.start
                } else if back {
                    let prev = step_cluster(doc, h, dir)?;
                    paragraph(doc, &self.outline, prev)?.1.start
                } else {
                    step_cluster(doc, para.end, dir)?.max(para.end)
                }
            }
            Granularity::Document if back => caret_gap(doc, 0)?,
            Granularity::Document => self.last_gap()?,
        })
    }

    /// Moves the caret, or extends the selection (`extend`, Shift held). Vertical moves keep
    /// the x the first of them started from, as Word does.
    pub fn move_caret(
        &mut self,
        g: Granularity,
        dir: Direction,
        extend: bool,
    ) -> Result<Selection, EditError> {
        let range = self.selection.range(&self.doc)?;
        let selection = if !extend && !range.is_empty() && g == Granularity::Char {
            let gap = if dir == Direction::Backward {
                range.start
            } else {
                range.end
            };
            Selection::caret(self.at(gap)?)
        } else {
            let h = self.selection.head.index(&self.doc)?;
            let gap = self.target(g, dir, h)?;
            let head = self.at(gap)?;
            if extend {
                Selection {
                    anchor: self.selection.anchor,
                    head,
                }
            } else {
                Selection::caret(head)
            }
        };
        if g != Granularity::Line {
            self.goal_x = None;
        }
        self.pending = None;
        self.history.break_group();
        self.selection = self.normalise(selection)?;
        Ok(self.selection)
    }

    /// Selects the word at `pos` (double click).
    pub fn select_word(&mut self, pos: Position) -> Result<Selection, EditError> {
        let gap = pos.index(&self.doc)?;
        let (_, para) = paragraph(&self.doc, &self.outline, gap)?;
        let r = word_at(&self.doc, gap, &para);
        self.select(r.start, r.end)
    }

    /// Selects the paragraph at `pos` (triple click).
    pub fn select_paragraph(&mut self, pos: Position) -> Result<Selection, EditError> {
        let (_, para) = paragraph(&self.doc, &self.outline, pos.index(&self.doc)?)?;
        self.select(para.start, para.end)
    }

    fn select(&mut self, a: usize, b: usize) -> Result<Selection, EditError> {
        self.selection = Selection {
            anchor: self.at(a)?,
            head: self.at(b)?,
        };
        self.pending = None;
        self.goal_x = None;
        self.history.break_group();
        Ok(self.selection)
    }

    /// Opens a composition at the caret, deleting the selection first (one undo step).
    pub fn composition_start(&mut self) -> Result<Applied, EditError> {
        let applied = if self.selection.is_collapsed(&self.doc)? {
            self.applied(None, Vec::new())
        } else {
            self.apply(Command::DeleteBackward(crate::commands::DeleteUnit::Char))?
        };
        self.composition = Some(Composition {
            at: self.selection.head,
            text: String::new(),
            caret: 0,
        });
        Ok(applied)
    }

    /// The composition's provisional text changed.
    pub fn composition_update(&mut self, text: &str, caret: usize) -> Result<(), EditError> {
        let c = self
            .composition
            .as_mut()
            .ok_or(EditError::NotApplicable("no composition is open"))?;
        text.clone_into(&mut c.text);
        c.caret = caret.min(text.chars().count());
        Ok(())
    }

    /// Inserts the composition's text as typing and closes it.
    pub fn composition_commit(&mut self) -> Result<Applied, EditError> {
        let c = self
            .composition
            .take()
            .ok_or(EditError::NotApplicable("no composition is open"))?;
        if c.text.is_empty() {
            return Ok(self.applied(None, Vec::new()));
        }
        self.selection = self.normalise(Selection::caret(c.at))?;
        self.apply(Command::InsertText(c.text))
    }

    /// Drops the composition.
    pub fn composition_cancel(&mut self) {
        self.composition = None;
    }

    /// The open composition, if any.
    #[must_use]
    pub const fn composition(&self) -> Option<&Composition> {
        self.composition.as_ref()
    }

    /// Does what `request` asks.
    pub fn handle(&mut self, request: Request) -> Result<Applied, EditError> {
        let unchanged = |e: &Self| e.applied(None, Vec::new());
        match request {
            Request::Command(c) => self.apply(c),
            Request::Undo => Ok(self.undo()?.unwrap_or_else(|| unchanged(self))),
            Request::Redo => Ok(self.redo()?.unwrap_or_else(|| unchanged(self))),
            Request::BreakUndoGroup => {
                self.break_undo_group();
                Ok(unchanged(self))
            }
            Request::MoveCaret {
                granularity,
                direction,
                extend,
            } => {
                self.move_caret(granularity, direction, extend)?;
                Ok(unchanged(self))
            }
            Request::SelectWord(p) => {
                self.select_word(p)?;
                Ok(unchanged(self))
            }
            Request::SelectParagraph(p) => {
                self.select_paragraph(p)?;
                Ok(unchanged(self))
            }
            Request::CompositionStart => self.composition_start(),
            Request::CompositionUpdate(text, caret) => {
                self.composition_update(&text, caret)?;
                Ok(unchanged(self))
            }
            Request::CompositionCommit => self.composition_commit(),
            Request::CompositionCancel => {
                self.composition_cancel();
                Ok(unchanged(self))
            }
            Request::SetViewport(pages) => {
                self.viewport = Some(pages);
                Ok(unchanged(self))
            }
        }
    }
}

//! The selection within stories: every selection stays in the story its anchor is in, and
//! leaving the body remembers the body selection to return to.

use super::Editor;
use crate::error::EditError;
use crate::position::{caret_gap, is_caret_gap, Direction, Position, Selection};
use crate::story::{Stories, Story};

impl Editor<'_> {
    /// `s` with each end moved to the nearest caret gap, and the head kept in the anchor's
    /// story (a selection never spans two: Word clamps it at the story's edge).
    pub(crate) fn normalise(&self, s: Selection) -> Result<Selection, EditError> {
        let fix = |p: Position| -> Result<Position, EditError> {
            let g = p.index(&self.doc)?;
            if is_caret_gap(&self.doc, g) {
                return Ok(p);
            }
            Position::at(&self.doc, caret_gap(&self.doc, g)?).ok_or(EditError::UnknownPosition)
        };
        let (anchor, head) = (fix(s.anchor)?, fix(s.head)?);
        let (a, h) = (anchor.index(&self.doc)?, head.index(&self.doc)?);
        let stories = Stories::new(&self.doc, &self.outline)?;
        let story = stories.at(a);
        if stories.at(h) == story {
            return Ok(Selection { anchor, head });
        }
        let toward_anchor = if h > a {
            Direction::Backward
        } else {
            Direction::Forward
        };
        let h = stories.clamp(story, h, toward_anchor).unwrap_or(a);
        Ok(Selection {
            anchor,
            head: Position::at(&self.doc, h).ok_or(EditError::UnknownPosition)?,
        })
    }

    /// The story the selection is in.
    pub fn story(&self) -> Result<Story, EditError> {
        let head = self.selection.head.index(&self.doc)?;
        Ok(Stories::new(&self.doc, &self.outline)?.at(head))
    }
}

impl Editor<'_> {
    /// Remembers `before` when the selection moved from the body to another story.
    pub(crate) fn left_body(
        &mut self,
        before: Selection,
        after: Selection,
    ) -> Result<(), EditError> {
        let stories = Stories::new(&self.doc, &self.outline)?;
        let story =
            |s: Selection| -> Result<Story, EditError> { Ok(stories.at(s.head.index(&self.doc)?)) };
        if story(after)? != Story::Body
            && before.head.index(&self.doc).is_ok()
            && story(before)? == Story::Body
        {
            self.body = Some(before);
        }
        Ok(())
    }

    /// The first and last caret gaps of the story gap `from` is in.
    pub(crate) fn story_ends(&self, from: usize) -> Result<(usize, usize), EditError> {
        let stories = Stories::new(&self.doc, &self.outline)?;
        stories
            .bounds(stories.at(from))
            .ok_or(EditError::UnknownPosition)
    }

    /// `gap`, reached moving `dir` from gap `from`, kept in `from`'s story: past the story's
    /// edge the caret stays at the edge; a body move skips the headers between sections.
    pub(crate) fn within_story(
        &self,
        from: usize,
        gap: usize,
        dir: Direction,
    ) -> Result<usize, EditError> {
        let stories = Stories::new(&self.doc, &self.outline)?;
        Ok(stories.clamp(stories.at(from), gap, dir).unwrap_or(from))
    }
}

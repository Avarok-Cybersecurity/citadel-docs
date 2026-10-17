//! Walking a run the way a lookup sees it: glyphs its flags ignore are stepped over, and
//! default ignorables (joiners, soft hyphens) are stepped over when they do not match.

use super::{Ctx, Table};
use crate::buffer::GlyphInfo;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Skip {
    No,
    Yes,
    Maybe,
}

/// What a glyph is to the walker.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Match,
    NoMatch,
    Skip,
}

/// A matcher that accepts whatever is not skipped.
pub(crate) const ANY: Option<&fn(u16, u16) -> bool> = None;

pub(crate) struct Walker {
    lookup_props: u32,
    ignore_zwnj: bool,
    ignore_zwj: bool,
    ignore_hidden: bool,
    pub(crate) idx: usize,
    data: u16,
}

impl Walker {
    /// A walker from `start`. `context` walks a rule's context, where joiners are always
    /// stepped over.
    pub(crate) fn new(ctx: &Ctx<'_, '_>, start: usize, context: bool) -> Self {
        let gpos = ctx.table == Table::Gpos;
        Self {
            lookup_props: ctx.lookup_props,
            ignore_zwnj: gpos || (context && ctx.auto_zwnj),
            ignore_zwj: context || ctx.auto_zwj,
            ignore_hidden: gpos,
            idx: start,
            data: 0,
        }
    }

    pub(crate) fn with_props(mut self, props: u32) -> Self {
        self.lookup_props = props;
        self
    }

    fn may_skip(&self, ctx: &Ctx<'_, '_>, info: &GlyphInfo) -> Skip {
        if !ctx.check_glyph_property(info, self.lookup_props) {
            return Skip::Yes;
        }
        if info.is_default_ignorable()
            && (self.ignore_zwnj || !info.is_zwnj())
            && (self.ignore_zwj || !info.is_zwj())
            && (self.ignore_hidden || !info.props.is_hidden())
        {
            return Skip::Maybe;
        }
        Skip::No
    }

    /// Whether the lookup's flags exclude `info` (as opposed to skipping it only when it does
    /// not match).
    pub(crate) fn skips_outright(&self, ctx: &Ctx<'_, '_>, info: &GlyphInfo) -> bool {
        self.may_skip(ctx, info) == Skip::Yes
    }

    /// Classifies `info`, with `matcher` given the glyph and the index of the rule element it
    /// is tried against.
    pub(crate) fn step<F: Fn(u16, u16) -> bool>(
        &self,
        ctx: &Ctx<'_, '_>,
        info: &GlyphInfo,
        matcher: Option<&F>,
    ) -> Step {
        let skip = self.may_skip(ctx, info);
        if skip == Skip::Yes {
            return Step::Skip;
        }
        match matcher.map(|m| m(info.glyph, self.data)) {
            Some(true) => Step::Match,
            None if skip == Skip::No => Step::Match,
            _ if skip == Skip::No => Step::NoMatch,
            _ => Step::Skip,
        }
    }

    /// Advances to the next matching glyph in the input.
    pub(crate) fn next<F: Fn(u16, u16) -> bool>(
        &mut self,
        ctx: &Ctx<'_, '_>,
        m: Option<&F>,
    ) -> bool {
        while self.idx + 1 < ctx.buf.len {
            self.idx += 1;
            match self.step(ctx, &ctx.buf.info[self.idx], m) {
                Step::Match => {
                    self.data += 1;
                    return true;
                }
                Step::NoMatch => return false,
                Step::Skip => {}
            }
        }
        false
    }

    /// Steps back to the previous matching glyph in the output.
    pub(crate) fn prev<F: Fn(u16, u16) -> bool>(
        &mut self,
        ctx: &Ctx<'_, '_>,
        m: Option<&F>,
    ) -> bool {
        while self.idx > 0 {
            self.idx -= 1;
            match self.step(ctx, ctx.buf.out_info(self.idx), m) {
                Step::Match => {
                    self.data += 1;
                    return true;
                }
                Step::NoMatch => return false,
                Step::Skip => {}
            }
        }
        false
    }
}

//! One lookup over the whole run. Glyphs the lookup cannot start on are passed in bulk, and a
//! pass over a run with no such glyph at all is skipped, which is where most of the time of a
//! plan with many narrow lookups (ccmp, locl, calt) would otherwise go.

use super::{Ctx, Table};
use crate::face::layout::Lookup;
use crate::plan::PlannedLookup;

impl Ctx<'_, '_> {
    /// Runs one planned lookup over the whole run.
    pub(crate) fn apply_string(&mut self, planned: PlannedLookup) {
        let face = self.face;
        let at = usize::from(planned.index);
        match self.table {
            Table::Gsub => {
                if let Some(lookup) = face.gsub.as_ref().and_then(|t| t.lookups.get(at)) {
                    self.run(planned, lookup, crate::substitute::apply);
                }
            }
            Table::Gpos => {
                if let Some(lookup) = face.gpos.as_ref().and_then(|t| t.lookups.get(at)) {
                    self.run(planned, lookup, crate::position::apply);
                }
            }
        }
    }

    fn run<T>(
        &mut self,
        planned: PlannedLookup,
        lookup: &Lookup<T>,
        apply: impl Fn(&mut Self, &T) -> bool,
    ) {
        let len = self.buf.len;
        if !self.buf.info[..len]
            .iter()
            .any(|i| lookup.glyphs.contains(i.glyph))
        {
            return;
        }
        self.lookup_props = lookup.props;
        self.auto_zwj = planned.auto_zwj;
        self.auto_zwnj = planned.auto_zwnj;
        self.last_base = None;
        self.last_base_until = 0;
        let once = |ctx: &mut Self| {
            let applied = ctx.check_glyph_property(ctx.buf.cur(0), lookup.props)
                && lookup.applies(ctx.buf.cur(0).glyph, |s| apply(ctx, s));
            ctx.lookup_props = lookup.props;
            applied
        };
        if lookup.reverse {
            self.buf.idx = len - 1;
            loop {
                if lookup.glyphs.contains(self.buf.cur(0).glyph) {
                    once(self);
                }
                if self.buf.idx == 0 {
                    return;
                }
                self.buf.idx -= 1;
            }
        }
        let substituting = self.table == Table::Gsub;
        if substituting {
            self.buf.clear_output();
        }
        self.buf.idx = 0;
        while self.buf.idx < self.buf.len && !self.buf.failed {
            let skip = self.buf.info[self.buf.idx..self.buf.len]
                .iter()
                .take_while(|i| !lookup.glyphs.contains(i.glyph))
                .count();
            if skip > 0 {
                self.buf.next_glyphs(skip);
                continue;
            }
            if !once(self) {
                self.buf.next_glyph();
            }
        }
        if substituting {
            self.buf.sync();
        }
    }
}

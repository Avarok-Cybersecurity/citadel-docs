//! The shaper: a face, a cache of plans, and buffers reused from one run to the next.

mod stages;

use crate::apply::{Ctx, Table};
use crate::buffer::Buffer;
use crate::error::ShapeError;
use crate::face::Face;
use crate::normalize::normalize;
use crate::params::{Direction, Script, ShapeParams};
use crate::plan::Plan;
use crate::position::{kern, propagate_attachments};
use crate::run::{Glyph, ShapedRun};

/// Shapes runs of text in one face. Plans are built per [`ShapeParams`] on first use and kept;
/// buffers are kept too, so shaping in a loop allocates nothing once warm.
pub struct Shaper<'f, 'a> {
    face: &'f Face<'a>,
    plans: Vec<(ShapeParams, Plan)>,
    buf: Buffer,
}

impl std::fmt::Debug for Shaper<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaper")
            .field("face", self.face)
            .field("plans", &self.plans.len())
            .finish_non_exhaustive()
    }
}

impl<'f, 'a> Shaper<'f, 'a> {
    /// A shaper for `face`, with no plans yet.
    #[must_use]
    pub fn new(face: &'f Face<'a>) -> Self {
        Self {
            face,
            plans: Vec::new(),
            buf: Buffer::new(),
        }
    }

    /// The face this shaper shapes with.
    #[must_use]
    pub fn face(&self) -> &'f Face<'a> {
        self.face
    }

    /// Shapes `text` as one run, returning a new [`ShapedRun`].
    pub fn shape(&mut self, text: &str, params: &ShapeParams) -> Result<ShapedRun, ShapeError> {
        let mut run = ShapedRun::empty();
        self.shape_into(text, params, &mut run)?;
        Ok(run)
    }

    /// Shapes `text` as one run into `run`, reusing its allocation. Clusters are byte offsets
    /// into `text`. On error `run` is left empty.
    pub fn shape_into(
        &mut self,
        text: &str,
        params: &ShapeParams,
        run: &mut ShapedRun,
    ) -> Result<(), ShapeError> {
        run.glyphs.clear();
        if params.script == Script::Unsupported {
            return Err(ShapeError::UnsupportedScript(params.script));
        }
        if params.direction != Direction::LeftToRight {
            return Err(ShapeError::UnsupportedDirection);
        }
        if u32::try_from(text.len()).is_err() {
            return Err(ShapeError::TextTooLong(text.len()));
        }
        let index = if let Some(i) = self.plans.iter().position(|(p, _)| p == params) {
            i
        } else {
            self.plans.push((*params, Plan::new(self.face, *params)));
            self.plans.len() - 1
        };
        let (face, plan, buf) = (self.face, &self.plans[index].1, &mut self.buf);
        run_pipeline(face, plan, buf, text);
        if buf.failed {
            return Err(ShapeError::Budget);
        }
        run.glyphs.extend(
            buf.info[..buf.len]
                .iter()
                .zip(&buf.pos[..buf.len])
                .map(|(i, p)| Glyph {
                    id: i.glyph,
                    cluster: i.cluster,
                    x_advance: p.x_advance,
                    y_advance: p.y_advance,
                    x_offset: p.x_offset,
                    y_offset: p.y_offset,
                }),
        );
        Ok(())
    }
}

fn run_pipeline(face: &Face<'_>, plan: &Plan, buf: &mut Buffer, text: &str) {
    buf.reset(text);
    stages::mark_continuations(buf);
    stages::form_clusters(buf);
    normalize(buf, face);
    stages::glyph_props(face, buf, plan.synthesize_classes);
    {
        let mut ctx = Ctx::new(face, buf, Table::Gsub);
        for stage in &plan.gsub {
            for lookup in stage {
                ctx.apply_string(*lookup);
            }
        }
    }
    if buf.failed {
        return;
    }
    stages::default_positions(face, buf);
    if plan.apply_gpos {
        let mut ctx = Ctx::new(face, buf, Table::Gpos);
        for lookup in &plan.gpos {
            ctx.apply_string(*lookup);
        }
    }
    if plan.apply_kern && plan.kern_requested {
        kern::apply(face, buf);
    }
    if plan.zero_marks {
        stages::zero_mark_widths(buf, plan.adjust_offsets_when_zeroing);
    }
    stages::zero_ignorable_widths(buf);
    propagate_attachments(buf);
    stages::hide_ignorables(face, buf);
}

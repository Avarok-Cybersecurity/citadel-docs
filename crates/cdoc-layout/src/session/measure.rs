//! The paragraph cache: a block measured and broken into strips once, and reused whenever its
//! content, the styles, the width and what it reads from the blocks before it (its list label,
//! the list counters a table's cells count on, the numbers of the footnotes it references)
//! are the same.

use super::hash::Fingerprint;
use super::record::Measured;
use super::run::{Pre, Run};
use crate::ir::Pt100;
use crate::LayoutError;
use cdoc_model::Block;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

/// What a measurement depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeasureKey {
    /// The block's fingerprint.
    pub content: u128,
    /// The style epoch: bumped whenever styles or numbering change.
    pub epoch: u64,
    /// The width it is broken to.
    pub width: Pt100,
    /// What it reads from before it.
    pub context: u128,
}

/// Measurements by key.
pub type Cache = BTreeMap<MeasureKey, Arc<Measured>>;

impl Run<'_, '_, '_> {
    /// Measures the blocks of `section` up to `upto`, in order, into the look-ahead.
    pub fn measure_to(&mut self, section: usize, upto: usize) -> Result<(), LayoutError> {
        while self.next + self.ahead.len() <= upto {
            let measured = self.measure(section, self.next + self.ahead.len())?;
            self.ahead.push_back(measured);
        }
        Ok(())
    }

    /// The counters and footnote numbering as they stand.
    pub fn pre(&self) -> Pre {
        (self.counters.counts().clone(), self.ctx.order.len())
    }

    fn measure(
        &mut self,
        section: usize,
        index: usize,
    ) -> Result<(Arc<Measured>, Pre), LayoutError> {
        let doc = self.ctx.doc;
        let pre = self.pre();
        let model = &doc.sections[section].blocks[index];
        let record = &self.records[section].blocks[index];
        let mut context = Fingerprint::new();
        let label = match model {
            Block::Paragraph(p) => {
                let label = self.counters.for_paragraph(doc, p)?;
                label.as_ref().map(|l| l.text.as_str()).hash(&mut context);
                label
            }
            Block::Table(_) => {
                self.counters.counts().hash(&mut context);
                None
            }
        };
        if !record.refs.is_empty() {
            self.ctx.notes.len().hash(&mut context);
            for id in &record.refs {
                self.ctx.notes.get(id).hash(&mut context);
            }
        }
        let key = MeasureKey {
            content: record.content,
            epoch: self.epoch,
            width: self.width,
            context: context.finish128(),
        };
        if let Some(hit) = self.cache.get(&key) {
            if let Some(counts) = &hit.counts_after {
                self.counters.restore(counts.clone());
            }
            for id in &hit.trace.notes {
                self.ctx.number_quietly(*id);
            }
            return Ok((Arc::clone(hit), pre));
        }
        let block = match model {
            Block::Paragraph(p) => {
                crate::paragraph::labelled(&mut self.ctx, p, self.width, label.as_ref())?
            }
            Block::Table(t) => {
                crate::table::table(&mut self.ctx, t, self.width, &mut self.counters)?
            }
        };
        let measured = Arc::new(Measured {
            block,
            trace: self.ctx.take_trace(),
            counts_after: matches!(model, Block::Table(_)).then(|| self.counters.counts().clone()),
        });
        self.cache.insert(key, Arc::clone(&measured));
        Ok((measured, pre))
    }
}

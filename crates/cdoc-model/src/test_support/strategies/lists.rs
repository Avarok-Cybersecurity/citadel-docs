//! Strategies for list definitions and instances.

use super::props::{indent, run_properties, ALIGNMENTS};
use super::{distinct_by, opt, pick};
use crate::ids::{AbstractNumId, NumId};
use crate::numbering::{
    AbstractNumbering, Level, LevelOverride, NumberFormat, Numbering, NumberingInstance,
};
use proptest::prelude::*;

pub(super) const FORMATS: &[NumberFormat] = &[
    NumberFormat::Decimal,
    NumberFormat::LowerLetter,
    NumberFormat::UpperLetter,
    NumberFormat::LowerRoman,
    NumberFormat::UpperRoman,
    NumberFormat::Bullet,
    NumberFormat::None,
];

fn level() -> BoxedStrategy<Level> {
    (
        0u32..10,
        pick(FORMATS),
        pick(&["%1.", "%1.%2", "•", "–"]).prop_map(str::to_owned),
        pick(ALIGNMENTS),
        opt(indent()),
        opt(run_properties()),
    )
        .prop_map(|(start, format, text, alignment, indent, run)| Level {
            start,
            format,
            text,
            alignment,
            indent,
            run,
        })
        .boxed()
}

/// Numbering with up to two definitions and two instances.
pub fn numbering() -> BoxedStrategy<Numbering> {
    let definition = (1u32..5, prop::collection::vec(level(), 1..=3)).prop_map(|(id, levels)| {
        AbstractNumbering {
            id: AbstractNumId(id),
            levels,
        }
    });
    let instance = (
        1u32..10,
        1u32..5,
        prop::collection::vec(
            (0u8..3, 0u32..10).prop_map(|(level, start)| LevelOverride { level, start }),
            0..=2,
        ),
    )
        .prop_map(|(id, abs, overrides)| NumberingInstance {
            id: NumId(id),
            abstract_id: AbstractNumId(abs),
            overrides,
        });
    (
        prop::collection::vec(definition, 0..=2),
        prop::collection::vec(instance, 0..=2),
    )
        .prop_map(|(definitions, instances)| Numbering {
            definitions: distinct_by(definitions, |d| d.id),
            instances: distinct_by(instances, |i| i.id),
        })
        .boxed()
}

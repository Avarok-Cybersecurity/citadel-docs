//! Proptest strategies. Sizes are small (a handful of sections, blocks and inlines) so a
//! property test runs in seconds; the shapes cover every enum variant and every optional field
//! both present and absent. Text is never empty, as a `ProseMirror` text node cannot be.

mod block;
mod document;
mod inline;
mod lists;
mod props;
mod units;

pub use block::{paragraph, table};
pub use document::{comment, document, footnote, section, style_sheet};
pub use inline::{inline, marks};
pub use lists::numbering;
pub use props::{borders, paragraph_properties, run_properties};
pub use units::{
    color, font_size, length, non_negative_length, positive_length, short_text, table_width,
    whole_points,
};

use proptest::prelude::*;

/// `items` without the later of any two that share a key: ids the model requires to be
/// distinct (`validate`) are distinct in every generated document.
pub fn distinct_by<T, K: Ord>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = std::collections::BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

/// `Some(value)` about half the time.
pub fn opt<T: std::fmt::Debug>(
    inner: impl Strategy<Value = T>,
) -> impl Strategy<Value = Option<T>> {
    prop::option::of(inner)
}

/// One of a fixed list of values.
pub fn pick<T: Clone + std::fmt::Debug + 'static>(
    values: &'static [T],
) -> impl Strategy<Value = T> {
    prop::sample::select(values)
}

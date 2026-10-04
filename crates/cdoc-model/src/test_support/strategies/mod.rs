//! Proptest strategies. Sizes are small (a handful of sections, blocks and inlines) so a
//! property test runs in seconds; the shapes cover every enum variant and every optional field
//! both present and absent. Text is never empty, as a `ProseMirror` text node cannot be.

mod block;
mod document;
mod inline;
mod props;
mod units;

pub use block::{paragraph, table};
pub use document::{comment, document, footnote, numbering, section, style_sheet};
pub use inline::{inline, marks};
pub use props::{borders, paragraph_properties, run_properties};
pub use units::{color, font_size, length, positive_length, short_text, table_width};

use proptest::prelude::*;

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

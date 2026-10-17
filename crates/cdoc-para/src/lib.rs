//! The paragraph builder: Knuth-Plass optimum-fit line breaking and Liang hyphenation, in
//! integers only, with no I/O, clock, randomness or global state.
//!
//! The crate knows nothing of fonts or documents. A caller measures its text, describes the
//! paragraph as TeX's [`Item`]s (boxes, glue, penalties and hyphens), and gets back where to
//! break and how far each line's glue is set. The Word-style layout and a TeX engine feed it
//! the same way.
//!
//! ```
//! use cdoc_para::{end_paragraph, Breaker, Item, KnuthPlass, LineWidths, Params};
//!
//! let mut items = Vec::new();
//! for (i, word) in [300; 6].into_iter().enumerate() {
//!     if i > 0 {
//!         items.push(Item::glue(100, 50, 33));
//!     }
//!     items.push(Item::boxed(word, i));
//! }
//! end_paragraph(&mut items);
//! let widths = LineWidths::uniform(1100)?;
//! let result = KnuthPlass.break_lines(&items, &widths, &Params::tex_plain())?;
//! assert_eq!(result.lines.len(), 2);
//! assert_eq!(result.lines[0].ratio, 0, "three words fill 1100 exactly");
//! # Ok::<(), cdoc_para::BreakError>(())
//! ```
//!
//! # Arithmetic
//!
//! Widths are `i32` in the caller's unit and summed in `i64`. Badness, fitness classes and
//! demerits are computed as `tex.web` computes them; adjustment ratios are fixed point in
//! 1/[`RATIO_ONE`]. [`justify`] shares a line's slack out by cumulative floors, so the glue
//! widths add up to the measure exactly. The same input gives the same output on every target.
//!
//! # Passes
//!
//! [`KnuthPlass`] runs TeX's passes: `pretolerance` without [`HyphenKind::Pattern`] breaks,
//! then `tolerance` with them, then (when `emergency_stretch > 0`) with extra stretch. When none
//! finds feasible breaks, the paragraph is set by [`FirstFit`] and the [`BreakReport`] says so.

#![forbid(unsafe_code)]

mod badness;
mod breaker;
mod error;
mod first_fit;
mod hyphen;
mod item;
mod justify;
mod knuth_plass;
mod measure;
mod params;
mod widths;

pub use badness::{badness, Fitness, INF_BAD, RATIO_ONE};
pub use breaker::{BreakReport, Breaker, Breaking, Line};
pub use error::{BreakError, PatternError};
pub use first_fit::FirstFit;
pub use hyphen::{HyphenMins, Hyphenator, Language, MAX_WORD};
pub use item::{end_paragraph, HyphenKind, Item, Order, Stretch, INFINITE_PENALTY};
pub use justify::{justify, Justified};
pub use knuth_plass::KnuthPlass;
pub use params::Params;
pub use widths::LineWidths;

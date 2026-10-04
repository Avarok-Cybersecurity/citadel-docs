//! What the backends take from the world: image bytes and the time. Images come through
//! `cdoc_model::ports::ImagePort` (re-exported here, shared with the `.docx` writer); fonts
//! through `cdoc_layout::FontPort`, the same port the layout measured with.

pub use cdoc_model::ports::{ImageBytes, ImageError, ImageLibrary, ImagePort};

/// The time, for the dates a PDF records about itself. Only the caller has a clock.
pub trait ClockPort {
    /// Now, as RFC 3339 (`2026-10-04T12:00:00Z`).
    fn now_rfc3339(&self) -> String;
}

/// A clock that always says the same time: what the WASM facade builds from the time the
/// browser passed in, and what tests use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedClock(pub String);

impl ClockPort for FixedClock {
    fn now_rfc3339(&self) -> String {
        self.0.clone()
    }
}

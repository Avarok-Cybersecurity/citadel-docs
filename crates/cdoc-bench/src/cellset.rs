//! Which cells, and from whom, a raw file must hold. The browser competitors are named here once:
//! owner decision 2026-10-18, Word (web) and Google Docs are not compared (no test accounts), so
//! a raw file from them, or from any product not listed, is refused rather than filed.

use crate::schema::Provenance;

/// The browser products the gate compares ours with, by the `tool` name their runner writes.
pub const BROWSER_COMPETITORS: [&str; 1] = ["ONLYOFFICE Docs"];

/// Which cells a raw file must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSet {
    /// Whatever the runner measures (the native script runners).
    Any,
    /// Exactly [`crate::raw::browser_cells`], from a browser product (`Provenance::Recorded`).
    Browser,
    /// Exactly [`crate::raw::browser_cells`], from ours in the browser (`Provenance::RecordedOurs`, both
    /// commits full ids).
    BrowserOurs,
}

impl CellSet {
    /// `any`, `browser` or `browser-ours`.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "any" => Ok(Self::Any),
            "browser" => Ok(Self::Browser),
            "browser-ours" => Ok(Self::BrowserOurs),
            other => Err(format!(
                "cell set {other:?}: expected any, browser or browser-ours"
            )),
        }
    }

    /// Whether `p` is the provenance this set requires.
    pub(crate) fn admits(self, p: &Provenance) -> bool {
        let full = |c: &str| c.len() == 40 && c.bytes().all(|b| b.is_ascii_hexdigit());
        match (self, p) {
            (Self::Any, _) | (Self::Browser, Provenance::Recorded { .. }) => true,
            (
                Self::BrowserOurs,
                Provenance::RecordedOurs {
                    ui_commit,
                    engine_commit,
                    ..
                },
            ) => full(ui_commit) && full(engine_commit),
            _ => false,
        }
    }
}

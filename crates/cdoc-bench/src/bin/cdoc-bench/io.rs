//! The harness's I/O: the monotonic clock, fixture files, JSON in and out.

use cdoc_bench::metrics::Fixture;
use cdoc_bench::stats::Clock;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The process's monotonic clock.
pub struct Monotonic(Instant);

impl Monotonic {
    pub fn start() -> Self {
        Self(Instant::now())
    }
}

impl Clock for Monotonic {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// The three fixture files of a page count, by extension.
pub fn fixture_path(dir: &Path, pages: u32, ext: &str) -> PathBuf {
    dir.join(format!("{pages}p.{ext}"))
}

pub fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_str(&read_text(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    write(path, format!("{text}\n").as_bytes())
}

/// Loads the fixtures of `pages` from `dir`.
pub fn load_fixtures(dir: &Path, pages: &[u32]) -> Result<Vec<Fixture>, String> {
    pages
        .iter()
        .map(|&p| {
            let doc = read_json(&fixture_path(dir, p, "json"))?;
            Ok(Fixture {
                pages: p,
                doc,
                docx: read_bytes(&fixture_path(dir, p, "docx"))?,
                md: read_text(&fixture_path(dir, p, "md"))?,
            })
        })
        .collect()
}

/// Every `*.json` in `dir`, sorted by name.
pub fn json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.extension().is_some_and(|x| x == "json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

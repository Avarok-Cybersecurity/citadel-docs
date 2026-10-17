//! Splitting text into script runs. Each character's script comes from Unicode `Scripts.txt`
//! (version [`crate::UNICODE_VERSION`]); characters of no script of their own (spaces,
//! digits, punctuation: `Common`; combining marks: `Inherited`; unassigned: `Unknown`) join the
//! run before them, or the first run after them at the start of the text. Text with no script
//! at all is one `Common` run.

use crate::params::Script;
use crate::unicode::{script, CharScript};
use std::ops::Range;

/// A maximal stretch of text in one script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRun {
    /// Byte range in the text.
    pub range: Range<usize>,
    /// The run's script; [`Script::Unsupported`] for scripts this release does not shape.
    pub script: Script,
}

fn strong(c: char) -> Option<Script> {
    match script(c) {
        CharScript::Latin => Some(Script::Latin),
        CharScript::Greek => Some(Script::Greek),
        CharScript::Cyrillic => Some(Script::Cyrillic),
        CharScript::Other => Some(Script::Unsupported),
        CharScript::Common | CharScript::Inherited => None,
    }
}

/// The script runs of `text`, in order, covering it exactly.
#[must_use]
pub fn itemise(text: &str) -> Vec<ScriptRun> {
    let first = text.chars().find_map(strong).unwrap_or(Script::Common);
    let mut runs: Vec<ScriptRun> = Vec::new();
    let mut current = first;
    for (at, c) in text.char_indices() {
        if let Some(s) = strong(c) {
            current = s;
        }
        let end = at + c.len_utf8();
        match runs.last_mut() {
            Some(run) if run.script == current => run.range.end = end,
            _ => runs.push(ScriptRun {
                range: at..end,
                script: current,
            }),
        }
    }
    runs
}

//! Character properties one key at a time: the keys a property set holds, and the marks that
//! turn one formatting into another.

use cdoc_crdt::{Mark, RunKey};
use cdoc_model::{Marks, RunProperties};

/// Every character-property key with its value in `p`.
pub(crate) fn fields(p: &RunProperties) -> [(RunKey, RunProperties); 12] {
    let none = RunProperties::none;
    [
        (
            RunKey::Style,
            RunProperties {
                style: p.style.clone(),
                ..none()
            },
        ),
        (
            RunKey::Font,
            RunProperties {
                font: p.font.clone(),
                ..none()
            },
        ),
        (
            RunKey::Size,
            RunProperties {
                size: p.size,
                ..none()
            },
        ),
        (
            RunKey::Bold,
            RunProperties {
                bold: p.bold,
                ..none()
            },
        ),
        (
            RunKey::Italic,
            RunProperties {
                italic: p.italic,
                ..none()
            },
        ),
        (
            RunKey::Underline,
            RunProperties {
                underline: p.underline.clone(),
                ..none()
            },
        ),
        (
            RunKey::Strike,
            RunProperties {
                strike: p.strike,
                ..none()
            },
        ),
        (
            RunKey::VerticalAlign,
            RunProperties {
                vertical_align: p.vertical_align,
                ..none()
            },
        ),
        (
            RunKey::Color,
            RunProperties {
                color: p.color,
                ..none()
            },
        ),
        (
            RunKey::Highlight,
            RunProperties {
                highlight: p.highlight,
                ..none()
            },
        ),
        (
            RunKey::SmallCaps,
            RunProperties {
                small_caps: p.small_caps,
                ..none()
            },
        ),
        (
            RunKey::AllCaps,
            RunProperties {
                all_caps: p.all_caps,
                ..none()
            },
        ),
    ]
}

/// The keys `p` sets.
pub(crate) fn keys(p: &RunProperties) -> Vec<RunKey> {
    fields(p)
        .into_iter()
        .filter(|(_, v)| !v.is_none())
        .map(|(k, _)| k)
        .collect()
}

/// The edits that turn `have` into `want`, one character-property key at a time.
pub(crate) fn run_marks(have: &RunProperties, want: &RunProperties) -> Vec<Mark> {
    let mut set = RunProperties::none();
    let mut out = Vec::new();
    for ((key, h), (_, w)) in fields(have).into_iter().zip(fields(want)) {
        if h == w {
            continue;
        }
        if w.is_none() {
            out.push(Mark::ClearRun(key));
        } else {
            set = set.overlay(&w);
        }
    }
    if !set.is_none() {
        out.push(Mark::Run(set));
    }
    out
}

/// The edits that turn the marks `have` into `want`.
pub(crate) fn mark_edits(have: &Marks, want: &Marks) -> Vec<Mark> {
    let mut out = Vec::new();
    if have.link != want.link {
        out.push(Mark::Link(want.link.clone()));
    }
    if have.revision != want.revision {
        out.push(Mark::Revision(want.revision.clone()));
    }
    for id in &have.comments {
        if !want.comments.contains(id) {
            out.push(Mark::Comment {
                id: id.clone(),
                anchored: false,
            });
        }
    }
    for id in &want.comments {
        if !have.comments.contains(id) {
            out.push(Mark::Comment {
                id: id.clone(),
                anchored: true,
            });
        }
    }
    out
}

/// Removes `key`'s value from `p`.
pub(crate) fn clear_key(p: &mut RunProperties, key: RunKey) {
    match key {
        RunKey::Style => p.style = None,
        RunKey::Font => p.font = None,
        RunKey::Size => p.size = None,
        RunKey::Bold => p.bold = None,
        RunKey::Italic => p.italic = None,
        RunKey::Underline => p.underline = None,
        RunKey::Strike => p.strike = None,
        RunKey::VerticalAlign => p.vertical_align = None,
        RunKey::Color => p.color = None,
        RunKey::Highlight => p.highlight = None,
        RunKey::SmallCaps => p.small_caps = None,
        RunKey::AllCaps => p.all_caps = None,
    }
}

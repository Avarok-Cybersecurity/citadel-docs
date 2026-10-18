//! Character properties one key at a time: the keys a property set holds, and the marks that
//! turn one formatting into another.

use cdoc_crdt::{Mark, RunKey};
use cdoc_model::{CommentId, Marks, RunProperties};

/// Every character-property key with its value in `p` (one entry per [`RunKey::ALL`], so a
/// key added there does not compile until it is listed here).
pub(crate) fn fields(p: &RunProperties) -> [(RunKey, RunProperties); RunKey::ALL.len()] {
    let only = |copy: fn(&mut RunProperties, &RunProperties)| {
        let mut one = RunProperties::none();
        copy(&mut one, p);
        one
    };
    [
        (RunKey::Style, only(|o, p| o.style.clone_from(&p.style))),
        (RunKey::Font, only(|o, p| o.font.clone_from(&p.font))),
        (RunKey::Size, only(|o, p| o.size = p.size)),
        (RunKey::Bold, only(|o, p| o.bold = p.bold)),
        (RunKey::Italic, only(|o, p| o.italic = p.italic)),
        (
            RunKey::Underline,
            only(|o, p| o.underline.clone_from(&p.underline)),
        ),
        (RunKey::Strike, only(|o, p| o.strike = p.strike)),
        (
            RunKey::VerticalAlign,
            only(|o, p| o.vertical_align = p.vertical_align),
        ),
        (RunKey::Color, only(|o, p| o.color = p.color)),
        (RunKey::Highlight, only(|o, p| o.highlight = p.highlight)),
        (RunKey::SmallCaps, only(|o, p| o.small_caps = p.small_caps)),
        (RunKey::AllCaps, only(|o, p| o.all_caps = p.all_caps)),
        (RunKey::Kern, only(|o, p| o.kern = p.kern)),
        (RunKey::Ligatures, only(|o, p| o.ligatures = p.ligatures)),
        (RunKey::Lang, only(|o, p| o.lang.clone_from(&p.lang))),
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
    let unanchor = |id: &CommentId| Mark::Comment {
        id: id.clone(),
        anchored: false,
    };
    out.extend(
        have.comments
            .iter()
            .filter(|id| !want.comments.contains(id))
            .map(unanchor),
    );
    // Comments read back in the order they were anchored, so everything from the first place
    // the order differs is anchored again, in `want`'s order.
    let kept = have.comments.iter().filter(|id| want.comments.contains(id));
    let same = kept.zip(&want.comments).take_while(|(h, w)| h == w).count();
    for id in &want.comments[same..] {
        if have.comments.contains(id) {
            out.push(unanchor(id));
        }
        out.push(Mark::Comment {
            id: id.clone(),
            anchored: true,
        });
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
        RunKey::Kern => p.kern = None,
        RunKey::Ligatures => p.ligatures = None,
        RunKey::Lang => p.lang = None,
    }
}

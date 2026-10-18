//! A session's relayout after an edit is exactly a fresh layout of the edited document, and
//! the pages it says did not change did not.

mod edits;
mod support;

use cdoc_layout::{
    layout, BlockEdit, BreakerChoice, Dirty, Layout, LayoutError, LayoutOptions, LayoutSession,
};
use cdoc_model::Document;
use cdoc_para::Params;
use proptest::prelude::*;

/// The first way two layouts differ, short enough to read in a failure.
fn difference(got: &Layout, want: &Layout) -> Option<String> {
    if got.pages.len() != want.pages.len() {
        return Some(format!(
            "{} pages, want {}",
            got.pages.len(),
            want.pages.len()
        ));
    }
    if let Some(i) = (0..got.pages.len()).find(|i| got.pages[*i] != want.pages[*i]) {
        let (g, w) = (&got.pages[i], &want.pages[i]);
        let at = (0..g.draws.len().max(w.draws.len())).find(|k| g.draws.get(*k) != w.draws.get(*k));
        return Some(format!(
            "page {i}: {}x{} vs {}x{}, draws {} vs {}, first differing {at:?}: {:?} vs {:?}; \
             links {} vs {}",
            g.width,
            g.height,
            w.width,
            w.height,
            g.draws.len(),
            w.draws.len(),
            at.and_then(|k| g.draws.get(k)),
            at.and_then(|k| w.draws.get(k)),
            g.links.len(),
            w.links.len()
        ));
    }
    let parts = [
        ("faces", got.faces != want.faces),
        ("outline", got.outline != want.outline),
        ("signatures", got.signatures != want.signatures),
        ("warnings", got.warnings != want.warnings),
        (
            "title",
            got.title != want.title || got.language != want.language,
        ),
    ];
    parts.iter().find(|(_, d)| *d).map(|(name, _)| match *name {
        "faces" => format!("faces {:?} vs {:?}", got.faces, want.faces),
        "warnings" => format!("warnings {:?} vs {:?}", got.warnings, want.warnings),
        other => (*other).to_owned(),
    })
}

/// Relayouts `doc` and compares it with a fresh layout; returns the new previous layout.
fn step(
    session: &mut LayoutSession<'_>,
    doc: &Document,
    dirty: &Dirty,
    previous: Option<&Layout>,
) -> Result<Option<Layout>, TestCaseError> {
    let fonts = support::fonts();
    let fresh = layout(
        doc,
        &fonts,
        &LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like())),
    );
    let got = session.relayout(doc, dirty);
    match (got, fresh) {
        (Ok(relayout), Ok(want)) => {
            let got = relayout.layout();
            if let Some(d) = difference(got, &want) {
                return Err(TestCaseError::fail(format!("after {dirty:?}: {d}")));
            }
            if let Some(previous) = previous {
                for (i, page) in got.pages.iter().enumerate() {
                    let said_unchanged = !relayout.changed_pages().contains(&i);
                    prop_assert!(
                        !said_unchanged || previous.pages.get(i) == Some(page),
                        "page {i} changed but was reported unchanged after {dirty:?}"
                    );
                }
            }
            Ok(Some(got.clone()))
        }
        (Err(a), Err(b)) => {
            prop_assert_eq!(a, b);
            Ok(None)
        }
        (got, want) => Err(TestCaseError::fail(format!(
            "session {:?} vs fresh {:?}",
            got.map(|_| "a layout"),
            want.map(|_| "a layout")
        ))),
    }
}

fn session_follows(mut doc: Document, ops: &[edits::Op]) -> Result<(), TestCaseError> {
    let fonts = support::fonts();
    let options = LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like()));
    let mut session = LayoutSession::new(&fonts, &options);
    let mut previous = step(&mut session, &doc, &Dirty::all(), None)?;
    for op in ops {
        let dirty = edits::apply(&mut doc, op);
        let dirty = if previous.is_some() {
            dirty
        } else {
            Dirty::all()
        };
        previous = step(&mut session, &doc, &dirty, previous.as_ref())?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn relayout_after_edits_is_a_fresh_layout(
        doc in edits::document(),
        ops in prop::collection::vec(edits::op(), 10..=16),
    ) {
        session_follows(doc, &ops)?;
    }
}

fn kitchen_sink_session(doc: &Document) -> (LayoutSession<'static>, Layout) {
    let fonts: &'static _ = Box::leak(Box::new(support::fonts()));
    let options: &'static _ = Box::leak(Box::new(LayoutOptions::word_like(
        BreakerChoice::KnuthPlass(Params::word_like()),
    )));
    let mut session = LayoutSession::new(fonts, options);
    let first = session
        .relayout(doc, &Dirty::all())
        .expect("lays out")
        .layout()
        .clone();
    (session, first)
}

#[test]
fn a_fresh_session_takes_only_a_full_relayout() {
    let fonts = support::fonts();
    let options = LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like()));
    let mut session = LayoutSession::new(&fonts, &options);
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let edit = Dirty::edits(vec![BlockEdit::changed(0, 0)]);
    assert_eq!(
        session.relayout(&doc, &edit).err(),
        Some(LayoutError::NoBaseline)
    );
    assert!(session.relayout(&doc, &Dirty::all()).is_ok());
}

#[test]
fn a_dirty_description_that_does_not_add_up_is_refused() {
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let (mut session, _) = kitchen_sink_session(&doc);
    let past_the_end = Dirty::edits(vec![BlockEdit::changed(0, 999)]);
    assert_eq!(
        session.relayout(&doc, &past_the_end).err(),
        Some(LayoutError::DirtyMismatch)
    );
    let (mut session, _) = kitchen_sink_session(&doc);
    let miscounted = Dirty::edits(vec![BlockEdit::inserted(0, 0, 1)]);
    assert_eq!(
        session.relayout(&doc, &miscounted).err(),
        Some(LayoutError::DirtyMismatch)
    );
}

#[test]
fn nothing_changed_repaints_nothing() {
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let (mut session, first) = kitchen_sink_session(&doc);
    let again = session
        .relayout(&doc, &Dirty::edits(Vec::new()))
        .expect("lays out");
    assert_eq!(again.changed_pages(), [0usize; 0]);
    assert_eq!(again.layout(), &first);
}

#[test]
fn an_edit_on_the_last_page_repaints_only_that_page() {
    let mut doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let (mut session, first) = kitchen_sink_session(&doc);
    let last = doc.sections.len() - 1;
    let index = doc.sections[last].blocks.len() - 1;
    doc.sections[last].blocks[index] = support::para("the last words");
    let dirty = Dirty::edits(vec![BlockEdit::changed(last, index)]);
    let relayout = session.relayout(&doc, &dirty).expect("lays out");
    assert_eq!(relayout.layout(), &support::laid_out(&doc));
    assert_eq!(relayout.changed_pages(), [first.pages.len() - 1]);
}

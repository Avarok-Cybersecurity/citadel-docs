//! Strategies for sections, styles, numbering, notes and whole documents.

use super::block::block;
use super::lists::{numbering, FORMATS};
use super::props::{paragraph_properties, run_properties};
use super::units::{
    comment_id, non_negative_length, positive_length, rfc3339, short_text, style_id,
};
use super::{distinct_by, opt, pick};
use crate::ids::FootnoteId;
use crate::notes::{Comment, Footnote};
use crate::page::{Columns, Margins, Orientation, PageNumbering, PageSetup, PageSize};
use crate::section::{HeaderFooter, HeaderFooterSet, Section, SectionProperties, SectionStart};
use crate::style::{DocumentDefaults, Style, StyleKind, StyleSheet};
use crate::{Document, DocumentMeta, DocumentSettings};
use proptest::prelude::*;

fn page_setup() -> BoxedStrategy<PageSetup> {
    let size = (
        positive_length(),
        positive_length(),
        pick(&[Orientation::Portrait, Orientation::Landscape]),
    )
        .prop_map(|(width, height, orientation)| PageSize {
            width,
            height,
            orientation,
        });
    let margins = prop::collection::vec(non_negative_length(), 7..=7).prop_map(|m| Margins {
        top: m[0],
        bottom: m[1],
        left: m[2],
        right: m[3],
        header: m[4],
        footer: m[5],
        gutter: m[6],
    });
    let columns =
        (1u16..=3, non_negative_length(), any::<bool>()).prop_map(|(count, gap, separator)| {
            Columns {
                count,
                gap,
                separator,
            }
        });
    let numbering =
        (pick(FORMATS), opt(1u32..100)).prop_map(|(format, start)| PageNumbering { format, start });
    (size, margins, columns, numbering)
        .prop_map(|(size, margins, columns, page_numbering)| PageSetup {
            size,
            margins,
            columns,
            page_numbering,
        })
        .boxed()
}

fn header_footer_set() -> BoxedStrategy<HeaderFooterSet> {
    let hf = || prop::collection::vec(block(), 1..=2).prop_map(|blocks| HeaderFooter { blocks });
    (opt(hf()), opt(hf()), opt(hf()))
        .prop_map(|(default, first, even)| HeaderFooterSet {
            default,
            first,
            even,
        })
        .boxed()
}

/// A section with one to three blocks.
pub fn section() -> BoxedStrategy<Section> {
    let properties = (
        pick(&[
            SectionStart::NextPage,
            SectionStart::Continuous,
            SectionStart::EvenPage,
            SectionStart::OddPage,
        ]),
        page_setup(),
        header_footer_set(),
        header_footer_set(),
        any::<bool>(),
    )
        .prop_map(
            |(start, page, headers, footers, title_page)| SectionProperties {
                start,
                page,
                headers,
                footers,
                title_page,
            },
        );
    (properties, prop::collection::vec(block(), 1..=3))
        .prop_map(|(properties, blocks)| Section { properties, blocks })
        .boxed()
}

fn style() -> BoxedStrategy<Style> {
    (
        style_id(),
        short_text(),
        pick(&[StyleKind::Paragraph, StyleKind::Character, StyleKind::Table]),
        opt(style_id()),
        opt(style_id()),
        paragraph_properties(),
        run_properties(),
    )
        .prop_map(|(id, name, kind, based_on, next, paragraph, run)| Style {
            id,
            name,
            kind,
            based_on,
            next,
            paragraph,
            run,
        })
        .boxed()
}

/// A style sheet with up to three styles.
pub fn style_sheet() -> BoxedStrategy<StyleSheet> {
    let defaults = (run_properties(), paragraph_properties())
        .prop_map(|(run, paragraph)| DocumentDefaults { run, paragraph });
    (defaults, prop::collection::vec(style(), 0..=3))
        .prop_map(|(defaults, styles)| StyleSheet {
            defaults,
            styles: distinct_by(styles, |s| s.id.clone()),
        })
        .boxed()
}

/// A footnote with one or two blocks.
pub fn footnote() -> BoxedStrategy<Footnote> {
    (1u32..20, prop::collection::vec(block(), 1..=2))
        .prop_map(|(id, blocks)| Footnote {
            id: FootnoteId(id),
            blocks,
        })
        .boxed()
}

/// A comment, sometimes a reply.
pub fn comment() -> BoxedStrategy<Comment> {
    (
        comment_id(),
        short_text(),
        rfc3339(),
        short_text(),
        any::<bool>(),
        opt(comment_id()),
    )
        .prop_map(|(id, author, date, text, resolved, parent)| Comment {
            id,
            author,
            date,
            text,
            resolved,
            parent,
        })
        .boxed()
}

/// A whole document: one or two sections, up to two footnotes and comments.
pub fn document() -> BoxedStrategy<Document> {
    let meta = (opt(short_text()), opt(Just("en-GB".to_owned())))
        .prop_map(|(title, language)| DocumentMeta { title, language });
    let settings = (
        any::<bool>(),
        any::<bool>(),
        positive_length(),
        any::<bool>(),
    )
        .prop_map(
            |(even_and_odd_headers, track_changes, default_tab_stop, auto_hyphenation)| {
                DocumentSettings {
                    even_and_odd_headers,
                    track_changes,
                    default_tab_stop,
                    auto_hyphenation,
                }
            },
        );
    (
        meta,
        settings,
        style_sheet(),
        numbering(),
        prop::collection::vec(section(), 1..=2),
        prop::collection::vec(footnote(), 0..=2),
        prop::collection::vec(comment(), 0..=2),
    )
        .prop_map(
            |(meta, settings, styles, numbering, sections, footnotes, comments)| Document {
                meta,
                settings,
                styles,
                numbering,
                sections,
                footnotes,
                comments: distinct_by(comments, |c| c.id.clone()),
            },
        )
        .boxed()
}

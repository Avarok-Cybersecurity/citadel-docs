//! The document's auxiliary parts: styles, numbering, settings, footnotes, comments.

use super::Writer;
use crate::error::DocxError;
use crate::ns::{content_type as ct, declare};
use crate::package::rel_type;
use crate::{numbering, settings, styles};
use cdoc_model::Document;

impl Writer<'_> {
    pub(super) fn aux_parts(&mut self, doc: &Document) -> Result<(), DocxError> {
        self.part("word/styles.xml", ct::STYLES, |w| {
            Ok(declare(styles::write_styles(&doc.styles, &mut w.log)))
        })?;
        self.rels.add(rel_type::STYLES, "styles.xml", false);
        if !doc.numbering.definitions.is_empty() || !doc.numbering.instances.is_empty() {
            self.part("word/numbering.xml", ct::NUMBERING, |w| {
                Ok(declare(numbering::write_numbering(
                    &doc.numbering,
                    &mut w.log,
                )))
            })?;
            self.rels.add(rel_type::NUMBERING, "numbering.xml", false);
        }
        self.part("word/settings.xml", ct::SETTINGS, |w| {
            Ok(declare(settings::write_settings(
                &doc.settings,
                !doc.footnotes.is_empty(),
                &mut w.log,
            )))
        })?;
        self.rels.add(rel_type::SETTINGS, "settings.xml", false);
        if !doc.footnotes.is_empty() {
            self.part("word/footnotes.xml", ct::FOOTNOTES, |w| w.footnotes(doc))?;
            self.rels.add(rel_type::FOOTNOTES, "footnotes.xml", false);
        }
        if !doc.comments.is_empty() {
            self.part("word/comments.xml", ct::COMMENTS, |w| Ok(w.comments(doc)))?;
            self.rels.add(rel_type::COMMENTS, "comments.xml", false);
            self.part("word/commentsExtended.xml", ct::COMMENTS_EXTENDED, |w| {
                Ok(w.comments_extended(doc))
            })?;
            self.rels
                .add(rel_type::COMMENTS_EXTENDED, "commentsExtended.xml", false);
        }
        Ok(())
    }
}

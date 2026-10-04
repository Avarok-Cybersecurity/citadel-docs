//! Content controls inside a paragraph.

use super::paragraph::{Context, ParaState};
use super::Reader;
use crate::error::DocxError;
use crate::tag::Tagged;
use crate::xml::Node;
use cdoc_model::{Inline, InlineContent};

impl Reader<'_> {
    pub(super) fn sdt_inline(
        &mut self,
        sdt: &Node,
        ctx: &Context,
        st: &mut ParaState,
    ) -> Result<(), DocxError> {
        let tag = sdt
            .find("w:sdtPr")
            .and_then(|pr| pr.find("w:tag"))
            .and_then(Node::value)
            .and_then(Tagged::decode);
        if let Some(tagged) = tag {
            let content = match tagged {
                Tagged::Form(f) => InlineContent::FormField(f),
                Tagged::Signature(f) => InlineContent::SignatureField(f),
            };
            st.inlines.push(Inline {
                content,
                marks: st.marks(ctx),
            });
            return Ok(());
        }
        self.log.lose("content control unwrapped to its content");
        if let Some(content) = sdt.find("w:sdtContent") {
            self.walk(content, ctx, st)?;
        }
        Ok(())
    }
}

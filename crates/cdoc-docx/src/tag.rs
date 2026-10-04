//! The content-control tag that carries a form or signature field losslessly. Word shows the
//! native control (a drop-down, a check box); the tag holds the model value so reading it back
//! is exact, whatever the control cannot express.

use cdoc_model::{FormField, SignatureField};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "cdoc:";

/// A field as stored in a content-control tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Tagged {
    /// A form field.
    Form(FormField),
    /// A signature placeholder.
    Signature(SignatureField),
}

impl Tagged {
    /// The tag text.
    #[must_use]
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).expect("model values serialise");
        format!("{PREFIX}{json}")
    }

    /// Parse a tag; `None` when it is not one of ours or is damaged.
    #[must_use]
    pub fn decode(tag: &str) -> Option<Self> {
        serde_json::from_str(tag.strip_prefix(PREFIX)?).ok()
    }
}

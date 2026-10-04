//! Form fields and signature fields.

use crate::ids::FieldId;
use crate::units::Length;
use serde::{Deserialize, Serialize};

/// What a form field collects.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormFieldKind {
    /// Free text.
    Text {
        /// The current value (may be empty).
        value: String,
        /// Maximum length in characters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<u32>,
    },
    /// A check box.
    Checkbox {
        /// Whether it is ticked.
        checked: bool,
    },
    /// A drop-down list.
    Dropdown {
        /// The choices; never empty.
        options: Vec<String>,
        /// Index of the chosen option.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<u32>,
    },
    /// A date picker.
    Date {
        /// The chosen date as `YYYY-MM-DD`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
}

/// A fillable field in the body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    /// Unique within the document.
    pub id: FieldId,
    /// A name for forms processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The field kind and value.
    pub field: FormFieldKind,
}

/// Where a field lands on a laid-out page, in page coordinates: origin at the top-left of the
/// page, y downwards. Filled in by the layout crate; the signing crate reads it to place a
/// signature appearance in the PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAnchor {
    /// Zero-based page index.
    pub page: u32,
    /// Left edge.
    pub x: Length,
    /// Top edge.
    pub y: Length,
    /// Width.
    pub width: Length,
    /// Height.
    pub height: Length,
}

/// A signature placeholder. The signing track owns the envelope, recipients and the signature
/// itself; the model only records where the field is and who is meant to sign it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureField {
    /// Unique within the document.
    pub id: FieldId,
    /// The role of the intended signer (e.g. `Client`, `Witness`); the envelope maps it to a
    /// person.
    pub signer_role: String,
    /// Text shown in the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Where it was laid out, once it has been.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<PageAnchor>,
}

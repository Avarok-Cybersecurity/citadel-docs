//! Reader options. There are no defaults: a Markdown file has no page, so the caller says how
//! wide the text area is and how large an image is drawn.

use cdoc_model::Length;

/// How to turn Markdown into a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MdReadOptions {
    /// Width of the text area; table columns share it equally.
    pub text_width: Length,
    /// Display width of an image (`![alt](cid:...)`); Markdown does not say.
    pub image_width: Length,
    /// Display height of an image.
    pub image_height: Length,
}

//! The ProseMirror node, mark and attribute names. The encoder, the decoder and the emitted
//! schema all read these constants, so a name cannot be spelled two ways. Attribute names that
//! equal a model field's serde name are not listed here: those come from the model itself.

/// Node type names.
pub mod node {
    /// The document.
    pub const DOC: &str = "doc";
    /// A section.
    pub const SECTION: &str = "section";
    /// A header inside a section.
    pub const HEADER: &str = "header";
    /// A footer inside a section.
    pub const FOOTER: &str = "footer";
    /// A paragraph.
    pub const PARAGRAPH: &str = "paragraph";
    /// A table.
    pub const TABLE: &str = "table";
    /// A table row.
    pub const TABLE_ROW: &str = "tableRow";
    /// A body cell.
    pub const TABLE_CELL: &str = "tableCell";
    /// A header cell.
    pub const TABLE_HEADER: &str = "tableHeader";
    /// Text.
    pub const TEXT: &str = "text";
    /// An inline image.
    pub const IMAGE: &str = "image";
    /// A line break.
    pub const HARD_BREAK: &str = "hardBreak";
    /// A page break.
    pub const PAGE_BREAK: &str = "pageBreak";
    /// A column break.
    pub const COLUMN_BREAK: &str = "columnBreak";
    /// A tab.
    pub const TAB: &str = "tab";
    /// A footnote reference mark.
    pub const FOOTNOTE_REF: &str = "footnoteRef";
    /// A computed field.
    pub const FIELD: &str = "field";
    /// A form field.
    pub const FORM_FIELD: &str = "formField";
    /// A signature field.
    pub const SIGNATURE_FIELD: &str = "signatureField";
    /// The container of footnote bodies at the end of the document.
    pub const FOOTNOTES: &str = "footnotes";
    /// One footnote body.
    pub const FOOTNOTE: &str = "footnote";
}

/// Mark type names.
pub mod mark {
    /// Bold, with `on`.
    pub const BOLD: &str = "bold";
    /// Italic, with `on`.
    pub const ITALIC: &str = "italic";
    /// Underline, with `style` and `color`.
    pub const UNDERLINE: &str = "underline";
    /// Strikethrough, with `on`.
    pub const STRIKE: &str = "strike";
    /// Super/subscript, with `value`.
    pub const VERTICAL_ALIGN: &str = "verticalAlign";
    /// Font family, size and colour.
    pub const TEXT_STYLE: &str = "textStyle";
    /// Highlight, with `color`.
    pub const HIGHLIGHT: &str = "highlight";
    /// Small caps, with `on`.
    pub const SMALL_CAPS: &str = "smallCaps";
    /// All caps, with `on`.
    pub const ALL_CAPS: &str = "allCaps";
    /// Kerning threshold in half-points (`w:kern`), with `value`.
    pub const KERN: &str = "kern";
    /// Ligature sets (`w14:ligatures`), with `value`.
    pub const LIGATURES: &str = "ligatures";
    /// Text language (`w:lang`), with `value`.
    pub const LANG: &str = "lang";
    /// A character style, with `styleId`.
    pub const CHARACTER_STYLE: &str = "characterStyle";
    /// A hyperlink, with `href` and `title`.
    pub const LINK: &str = "link";
    /// A comment anchor, with `id`; several may apply to one inline.
    pub const COMMENT: &str = "comment";
    /// A tracked insertion, with `id`, `author`, `date`.
    pub const INSERTION: &str = "insertion";
    /// A tracked deletion, with `id`, `author`, `date`.
    pub const DELETION: &str = "deletion";
}

/// Attribute names that do not coincide with a model field name.
pub mod attr {
    /// The boolean on `bold`, `italic`, `strike`, `smallCaps`, `allCaps`: `true` sets the
    /// property, `false` explicitly unsets an inherited one.
    pub const ON: &str = "on";
    /// The value on `verticalAlign`, `kern`, `ligatures` and `lang`.
    pub const VALUE: &str = "value";
    /// Font family on `textStyle` (the model field is `font`).
    pub const FONT_FAMILY: &str = "fontFamily";
    /// Font size on `textStyle`, in half-points (the model field is `size`).
    pub const FONT_SIZE: &str = "fontSize";
    /// Colour on `textStyle`, `highlight` and `underline`.
    pub const COLOR: &str = "color";
    /// Style id on `characterStyle`.
    pub const STYLE_ID: &str = "styleId";
    /// The id on `comment`, `insertion`, `deletion`, `footnoteRef`, `footnote`.
    pub const ID: &str = "id";
    /// Which page a header or footer applies to.
    pub const KIND: &str = "kind";
    /// Columns spanned, spelled as `prosemirror-tables` requires.
    pub const COLSPAN: &str = "colspan";
    /// Rows spanned, spelled as `prosemirror-tables` requires.
    pub const ROWSPAN: &str = "rowspan";
    /// Grid column widths on `table`.
    pub const COLUMN_WIDTHS: &str = "columnWidths";
    /// Document-level attribute holding the metadata.
    pub const META: &str = "meta";
    /// Document-level attribute holding the settings.
    pub const SETTINGS: &str = "settings";
    /// Document-level attribute holding the style sheet.
    pub const STYLES: &str = "styles";
    /// Document-level attribute holding the numbering.
    pub const NUMBERING: &str = "numbering";
    /// Document-level attribute holding the comments.
    pub const COMMENTS: &str = "comments";
}

//! `.docx` (OOXML `WordprocessingML`) reading and writing for the Citadel Docs model.
//!
//! Pure: bytes in, bytes out. Images are named by content id and move through the
//! [`cdoc_model::ImagePort`] and [`MediaSink`] ports. Anything the model or the format cannot carry is
//! listed in the [`Report`]; nothing is dropped silently.

#![forbid(unsafe_code)]

mod error;
mod ids;
mod kind;
mod ns;
mod numbering;
mod package;
mod props;
mod read;
mod report;
mod seal;
mod settings;
mod styles;
mod write;
mod xml;

pub use cdoc_model::{Loss, Report};
pub use cdoc_pkg::{
    repair_references, Imported, Kind, MediaSink, MemoryMedia, ReadLimits, WriteOptions,
};
pub use error::DocxError;
pub use read::{inherit_headers_and_footers, read_docx};
pub use write::write_docx;

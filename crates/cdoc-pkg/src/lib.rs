//! Plumbing shared by the zip-of-XML formats (`cdoc-docx`, `cdoc-odt`): the in-memory package,
//! a small XML tree with its parser and writer, the loss log the part readers share, the media
//! ports, the form-field tag, reference repair and the document/template kind.
//!
//! Format-neutral by construction: nothing here knows OPC relationships or ODF manifests.

#![forbid(unsafe_code)]

mod error;
mod kind;
mod log;
mod media;
mod repair;
pub mod tag;
pub mod xml;
mod zip;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use error::PackageError;
pub use kind::{Imported, Kind, WriteOptions};
pub use log::Log;
pub use media::{MediaSink, MemoryMedia};
pub use repair::repair_references;
pub use zip::{Compression, Lead, Package, ReadLimits};

/// A part's bytes, or [`PackageError::MissingPart`].
pub fn require<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], PackageError> {
    package
        .get(name)
        .ok_or_else(|| PackageError::MissingPart(name.to_owned()))
}

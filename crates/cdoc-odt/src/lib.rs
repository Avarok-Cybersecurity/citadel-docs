//! ODF text document (`.odt`, `.ott`) reading and writing for the Citadel Docs model.
//!
//! Pure: bytes in, bytes out. Images are named by content id and move through the
//! [`cdoc_model::ImagePort`] and [`MediaSink`] ports. Anything the model or the format cannot
//! carry is listed in the [`Report`]; nothing is dropped silently.

#![forbid(unsafe_code)]

mod datestyle;
mod lists;
mod ns;
mod page;
mod props;
mod read;
mod residual;
mod sidecar;
mod styles;
mod text;
mod units;
mod write;

pub use cdoc_model::{Loss, Report};
pub use cdoc_pkg::{
    Imported, Kind, MediaSink, MemoryMedia, PackageError as OdtError, ReadLimits, WriteOptions,
};
pub use read::read_odt;
pub use write::write_odt;

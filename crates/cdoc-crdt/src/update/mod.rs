//! Updates: the operations one replica holds and another lacks, in a compact binary form.

mod apply;
mod chunk;
mod decode;
mod encode;
pub(crate) mod ops;
pub(crate) mod wire;
mod writes;

pub use apply::Applied;
pub use chunk::ChunkError;
pub(crate) use encode::{ItemOut, OpSink};
pub(crate) use ops::Pending;
pub use writes::{update_writes, UpdateWrites};

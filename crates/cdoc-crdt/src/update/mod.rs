//! Updates: the operations one replica holds and another lacks, in a compact binary form.

mod apply;
mod chunk;
mod decode;
mod encode;
pub(crate) mod ops;
pub(crate) mod wire;

pub use apply::Applied;
pub use chunk::ChunkError;
pub(crate) use decode::read_op;
pub(crate) use encode::{OpSink, SnapshotOut};
pub(crate) use ops::Pending;

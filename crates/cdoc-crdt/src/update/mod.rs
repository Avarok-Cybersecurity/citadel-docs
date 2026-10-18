//! Updates: the operations one replica holds and another lacks, in a compact binary form.

mod apply;
mod blank;
mod chunk;
mod decode;
mod encode;
pub(crate) mod ops;
mod pending;
pub(crate) mod wire;
mod writes;

pub use apply::Applied;
pub use chunk::ChunkError;
pub(crate) use encode::{ItemOut, OpSink};
pub(crate) use pending::Pending;
pub use pending::PendingLimits;
pub use writes::{update_writes, UpdateWrites};

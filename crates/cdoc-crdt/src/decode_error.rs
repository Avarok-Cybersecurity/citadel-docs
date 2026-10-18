//! Why bytes were refused: the one error every decoder in this crate returns.

/// Bytes that are not a well-formed encoding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The input ended inside a value.
    #[error("unexpected end of input")]
    UnexpectedEnd,
    /// A varint longer than 64 bits.
    #[error("varint overflows 64 bits")]
    VarintOverflow,
    /// Text that is not UTF-8.
    #[error("invalid UTF-8")]
    InvalidUtf8,
    /// An unknown tag byte.
    #[error("unknown {what} tag {tag}")]
    BadTag {
        /// What was being read.
        what: &'static str,
        /// The tag found.
        tag: u8,
    },
    /// A record of another version than the one this build reads (the editor's records).
    #[error("{record} record version {found}; this build reads only version {supported}")]
    RecordVersion {
        /// Which record.
        record: &'static str,
        /// The version found.
        found: u8,
        /// The one version this build reads.
        supported: u8,
    },
    /// The input does not start with this crate's magic.
    #[error("not a cdoc-crdt encoding (bad magic)")]
    BadMagic,
    /// An encoding of another format version. Version 1 shipped deletions as an unclocked set
    /// that cannot be turned into version 2's clocked delete operations, so it is refused,
    /// never guessed at: re-encode it with the version that wrote it.
    #[error("cdoc-crdt format version {found}, this build reads only version {supported}")]
    Version {
        /// The version found.
        found: u8,
        /// The one version this build reads.
        supported: u8,
    },
    /// A snapshot in an earlier snapshot layout of this format version. Layout 1 wrote every
    /// id as an absolute clock; layout 2 locates units by position. Refused, never guessed at:
    /// re-encode it with the build that wrote it.
    #[error("snapshot layout {found}, this build reads only snapshot layout {supported}")]
    SnapshotLayout {
        /// The layout found.
        found: u8,
        /// The one layout this build reads.
        supported: u8,
    },
    /// The update needs history this replica or its sender has garbage-collected: the receiver
    /// is behind the sender's GC frontier, or the operation names a collected tombstone. The
    /// replica is unchanged; it must be reloaded from a snapshot.
    #[error("the update needs history compacted away by garbage collection")]
    Compacted,
    /// A value that does not fit the field it is read into.
    #[error("{0} out of range")]
    OutOfRange(&'static str),
    /// Values that are individually well formed but contradict each other.
    #[error("inconsistent encoding: {0}")]
    Inconsistent(&'static str),
    /// Bytes left over after the value.
    #[error("{0} trailing bytes")]
    Trailing(usize),
}

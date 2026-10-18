//! What a read accepts. Packages and their parts are untrusted input, so every bound is the
//! caller's explicit choice and a read that would exceed one fails before doing the work.

/// Bounds on what a read will inflate and parse, so a hostile package cannot exhaust memory,
/// time or stack.
///
/// Over a boundary it is JSON with every field required and unknown fields refused
/// (`{"maxParts", "maxPartBytes", "maxTotalBytes", "maxRatio", "ratioGraceBytes",
/// "maxDepth"}`): there is no default to fall back on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReadLimits {
    /// Most parts in the package.
    pub max_parts: usize,
    /// Largest single part, inflated.
    pub max_part_bytes: u64,
    /// Largest total, inflated.
    pub max_total_bytes: u64,
    /// Most bytes a part may inflate to per compressed byte, once it inflates past
    /// [`ReadLimits::ratio_grace_bytes`]: a zip bomb is a part far smaller than what it holds.
    pub max_ratio: u64,
    /// Size up to which a part's ratio is not checked (a small part of repeated markup can
    /// compress very well and is harmless).
    pub ratio_grace_bytes: u64,
    /// Deepest element nesting in a part. The tree is built without recursion, but the
    /// readers walking it recurse, and the WASM stack is small.
    pub max_depth: usize,
}

impl ReadLimits {
    /// 10 000 parts, 128 MiB per part, 512 MiB in all; a ratio of at most 100 past 100 KiB
    /// (Apache POI's `ZipSecureFile` bounds; real documents stay under 20); 256 levels of
    /// nesting. Well above any real document.
    #[must_use]
    pub const fn generous() -> Self {
        Self {
            max_parts: 10_000,
            max_part_bytes: 128 << 20,
            max_total_bytes: 512 << 20,
            max_ratio: 100,
            ratio_grace_bytes: 100 << 10,
            max_depth: 256,
        }
    }
}

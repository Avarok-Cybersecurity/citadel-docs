//! What the tests share.

/// Package import options with the read limits docs/edit-commands.md recommends for a
/// browser tab.
pub const BROWSER_READ: &str = r#"{"limits":{"maxParts":5000,"maxPartBytes":67108864,"maxTotalBytes":134217728,"maxRatio":100,"ratioGraceBytes":102400,"maxDepth":256}}"#;

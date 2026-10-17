//! Hooks for families whose crates are still landing: `crdt_*` (`cdoc-crdt`, against yrs).
//!
//! Each is a module behind a Cargo feature (`crdt`) that this crate does not declare until the
//! crate it measures is on `main`, so the module compiles only once that crate exists.
//! `build.rs` registers the feature name with `check-cfg`, so the gates stay warning-free
//! meanwhile. To land one:
//!
//! 1. add `cdoc-crdt = { workspace = true, optional = true }` and
//!    `crdt = ["dep:cdoc-crdt"]` to this crate's `Cargo.toml` (and its competitor, yrs, to the
//!    `competitors` feature);
//! 2. pass `--features crdt` in `.github/workflows/bench.yml`;
//! 3. add a plan entry per metric to `bench/plan.toml`;
//! 4. read the first runner artifact and add a budget per metric to `bench/budgets.toml`
//!    (the gate fails with "no budget for crdt_..." until then).

use super::{BenchError, Ctx, Fixture, Results};

#[cfg(feature = "crdt")]
#[path = "hooks_crdt.rs"]
mod crdt;
/// The `crdt_*` metric name prefix.
pub const CRDT_PREFIX: &str = "crdt_";

/// Runs every hooked family whose crate is enabled over `fixtures`; nothing while none is.
pub fn run(ctx: &Ctx<'_>, fixtures: &[Fixture], out: &mut Results) -> Result<(), BenchError> {
    #[cfg(feature = "crdt")]
    for fx in fixtures {
        crdt::run(ctx, fx, out)?;
    }
    let _ = (ctx, fixtures, out);
    Ok(())
}

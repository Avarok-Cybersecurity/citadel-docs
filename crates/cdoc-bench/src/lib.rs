//! The Citadel Docs benchmark harness: the R1 metrics and the corpus, defined once, the JSON
//! report, the markdown table, and the CI gate (budgets, and never slower than a measured
//! competitor).
//!
//! Bench and CI only: nothing that ships depends on this crate. The library is pure; the
//! `cdoc-bench` binary is the I/O edge (files, the monotonic clock, the process exit code).
//! See `docs/benchmarks.md`.

#![forbid(unsafe_code)]

pub mod budget;
pub mod corpus;
pub mod metrics;
pub mod plan;
pub mod prng;
pub mod schema;
pub mod stats;
pub mod table;

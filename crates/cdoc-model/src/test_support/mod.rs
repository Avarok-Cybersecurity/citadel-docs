//! Test support (feature `test-support`): proptest strategies for every model type and a
//! kitchen-sink fixture that uses every variant once. Dependent crates (`cdoc-pm`, the
//! exporters, the layout crate) use these for round-trip and golden tests so each does not
//! write its own generators. Never enabled by production code.

pub mod fixtures;
pub mod strategies;

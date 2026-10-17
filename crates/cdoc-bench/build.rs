//! Registers the hooked families' feature names (see `src/metrics/hooks.rs`) with `check-cfg`, so
//! their modules may be gated on features this crate declares only once their crates land.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(feature, values(\"crdt\"))");
}

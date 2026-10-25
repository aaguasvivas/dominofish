//! DominoFish core: rules, information sets, sampling, and search for
//! Dominican dominoes under Capi's rules. It has no I/O, global state,
//! threads, or clocks, and builds for wasm32-unknown-unknown.

pub mod rng;
pub mod rules;
pub mod tile;

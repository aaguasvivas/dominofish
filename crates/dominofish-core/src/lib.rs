//! DominoFish core: rules, information sets, sampling, and search for
//! Dominican dominoes under Capi's rules. It has no I/O, global state,
//! threads, or clocks, and builds for wasm32-unknown-unknown.

pub mod bench;
mod hash;
pub mod infoset;
pub mod perft;
#[cfg(test)]
mod properties;
pub mod record;
pub mod referee;
pub mod rng;
pub mod rules;
pub mod tile;

//! DominoFish core: rules, information sets, sampling, and search for
//! Dominican dominoes under Capi's rules. It has no I/O, global state,
//! threads, or clocks, and builds for wasm32-unknown-unknown.

pub mod bench;
pub mod bots;
mod hash;
pub mod infoset;
pub mod perft;
pub mod pimc;
pub mod position;
#[cfg(test)]
mod properties;
pub mod protocol;
pub mod record;
pub mod referee;
pub mod rng;
pub mod rules;
pub mod sampler;
pub mod solver;
pub mod tile;
pub mod worlds;

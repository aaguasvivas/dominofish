//! The playout bench export for wasm32-unknown-unknown (M1-10). No
//! wasm-bindgen: tools/wasm-bench.mjs loads the module with
//! WebAssembly.instantiate and calls this function directly.

use dominofish_core::bench::playouts;
use dominofish_core::rules::Mode;

/// The checksum of `rounds` seeded random playouts in mode 1 (1v1) or 2
/// (2v2); 0 for any other mode.
#[unsafe(no_mangle)]
pub extern "C" fn playouts_checksum(mode: u32, rounds: u32, seed: u32) -> u64 {
    let mode = match mode {
        1 => Mode::OneVsOne,
        2 => Mode::TwoVsTwo,
        _ => return 0,
    };
    playouts(mode, rounds, u64::from(seed)).0
}

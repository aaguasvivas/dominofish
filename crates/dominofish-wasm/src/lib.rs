//! The DominoFish core for wasm32-unknown-unknown, with no wasm-bindgen:
//! the playout bench export (M1-10, loaded by tools/wasm-bench.mjs) and the
//! web board's request interface (board.rs, loaded by web/app.js). Both
//! pages load the module with WebAssembly.instantiate.

mod board;

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

/// Reserves `len` bytes for a board request; the page writes the request's
/// UTF-8 text there and passes the pointer to `board_request`.
#[unsafe(no_mangle)]
pub extern "C" fn board_alloc(len: u32) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    core::mem::forget(buf);
    ptr
}

/// Handles one board request (board.rs) and frees its buffer. Returns the
/// reply as (pointer << 32) | length; the page reads it, then frees it with
/// `board_free`.
///
/// # Safety
///
/// `ptr` must come from `board_alloc(len)` and hold `len` written bytes; it
/// must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn board_request(ptr: *mut u8, len: u32) -> u64 {
    let input = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    let reply = board::handle(&String::from_utf8_lossy(&input));
    let reply = reply.into_bytes().into_boxed_slice();
    let n = reply.len() as u64;
    let p = Box::into_raw(reply) as *mut u8 as usize as u64;
    (p << 32) | n
}

/// Frees a reply from `board_request`.
///
/// # Safety
///
/// `ptr` and `len` must be a reply's pointer and length, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn board_free(ptr: *mut u8, len: u32) {
    drop(unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) });
}

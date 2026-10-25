//! M1-09: counts heap allocations, after setup, during move generation, play
//! and undo, hashing, perft, and random playouts in both modes. It runs
//! without the test harness (Cargo.toml), so only this workload is on the
//! process, and it exits 1 unless the count is 0.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

use dominofish_core::perft;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, MoveList, Phase, State};

struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

fn main() {
    // Setup: everything allocated before the counter is read.
    let mut rng = Rng::new(9);
    let mut moves = MoveList::new();
    let (mut games, mut plies, mut undone, mut nodes, mut hash) = (0u64, 0u64, 0u64, 0u64, 0u64);

    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for _ in 0..1000 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
            loop {
                match state.phase {
                    Phase::Finished => break,
                    Phase::RoundOver => state = state.next_round(&Deal::shuffled(&mut rng)),
                    Phase::Playing => {
                        state.legal_moves(&mut moves);
                        for &mv in moves.as_slice() {
                            let played = state.play(mv);
                            hash ^= state.hash() ^ state.recomputed_hash();
                            state.undo(mv, played.undo);
                            undone += 1;
                        }
                        if plies % 64 == 0 {
                            nodes += perft::lines(&mut state, 3);
                        }
                        let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                        state.play(mv);
                        plies += 1;
                    }
                }
            }
            games += 1;
        }
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;

    println!(
        "alloc: games {games} plies {plies} moves played and undone {undone} perft lines {nodes} allocations {allocations} (hash check {})",
        hash == 0
    );
    if allocations != 0 || hash != 0 {
        std::process::exit(1);
    }
}

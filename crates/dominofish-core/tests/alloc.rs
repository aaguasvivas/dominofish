//! M1-09: counts heap allocations, after setup, during move generation, play
//! and undo, hashing, perft, random playouts in both modes, and (M3) solver
//! nodes with the table and past the node cap, and (M4) sampled worlds. It
//! runs
//! without the test harness (Cargo.toml), so only this workload is on the
//! process, and it exits 1 unless the count is 0.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

mod common;

use dominofish_core::infoset::InfoSet;
use dominofish_core::perft;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, MoveList, Phase, State};
use dominofish_core::sampler::Sampler;
use dominofish_core::solver::{self, Table};

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
    let mut table = Table::new(16);
    let (mut solver_nodes, mut capped) = (0u64, 0u64);
    // Samplers at every fifth decision of seeded games in both modes.
    let mut samplers = Vec::new();
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        for seed in 0..10 {
            let record = common::random_record(mode, 100, 800_000 + seed, true);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for ply in 0..round.moves.len() {
                    if ply % 5 == 0 {
                        let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                        samplers.push(Sampler::new(&info).unwrap());
                    }
                    state.play(round.moves[ply]);
                }
            }
        }
    }
    let mut worlds = 0u64;

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
                        if plies % 16 == 0 {
                            // A cap of 300 nodes sends some solves to the
                            // fallback, so it is counted too.
                            table.clear();
                            let s = solver::solve(&mut state, Some(&mut table), 300);
                            solver_nodes += s.nodes;
                            capped += u64::from(s.capped);
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
    let mut world_rng = Rng::new(11);
    for sampler in &samplers {
        for _ in 0..20 {
            let w = sampler.sample(&mut world_rng);
            hash ^= w.state.hash() ^ w.state.recomputed_hash();
            worlds += 1;
        }
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;

    println!(
        "alloc: games {games} plies {plies} moves played and undone {undone} perft lines {nodes} solver nodes {solver_nodes} (capped solves {capped}) sampled worlds {worlds} allocations {allocations} (hash check {})",
        hash == 0
    );
    if allocations != 0 || hash != 0 {
        std::process::exit(1);
    }
}

//! Random playouts for the playout bench (M1-10): seeded rounds of uniformly
//! random canonical moves. The checksum fixes the exact work done, so a
//! native build and a wasm build can be compared on equal work.

use crate::rng::Rng;
use crate::rules::{Deal, Mode, MoveList, Phase, State};

/// Plays `rounds` independent rounds (a fresh deal, an empty board, the
/// leader rotating through the seats, target 100) and returns the checksum
/// over every final state's hash, and the number of plies.
pub fn playouts(mode: Mode, rounds: u32, seed: u64) -> (u64, u64) {
    let mut rng = Rng::new(seed);
    let mut moves = MoveList::new();
    let (mut checksum, mut plies) = (0u64, 0u64);
    for r in 0..rounds {
        let leader = (r % u32::from(mode.seats())) as u8;
        let mut state = State::dealt(mode, 100, [0, 0], &Deal::shuffled(&mut rng), leader);
        while state.phase == Phase::Playing {
            state.legal_moves(&mut moves);
            state.play(moves.as_slice()[rng.below(moves.len() as u32) as usize]);
            plies += 1;
        }
        checksum = checksum.rotate_left(5) ^ state.hash() ^ plies;
    }
    (checksum, plies)
}

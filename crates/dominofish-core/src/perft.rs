//! Perft: counts lines of canonical moves (docs/RULES.md, Perft). A line
//! ends at the given depth or where the round ends, whichever comes first.

use crate::rules::{Move, MoveList, Phase, State};

/// The number of lines from `state` to `depth` plies (u32::MAX: to the end
/// of the round). Leaves `state` as it found it.
pub fn lines(state: &mut State, depth: u32) -> u64 {
    if depth == 0 || state.phase != Phase::Playing {
        return 1;
    }
    let mut moves = MoveList::new();
    state.legal_moves(&mut moves);
    let mut n = 0;
    for &mv in moves.as_slice() {
        let played = state.play(mv);
        n += lines(state, depth - 1);
        state.undo(mv, played.undo);
    }
    n
}

/// `lines` split by root move: writes (move, lines below it) for each
/// canonical move and returns how many were written.
pub fn divide(state: &mut State, depth: u32, out: &mut [(Move, u64); 16]) -> usize {
    let mut moves = MoveList::new();
    state.legal_moves(&mut moves);
    for (slot, &mv) in out.iter_mut().zip(moves.as_slice()) {
        let played = state.play(mv);
        *slot = (mv, lines(state, depth.saturating_sub(1)));
        state.undo(mv, played.undo);
    }
    moves.len()
}

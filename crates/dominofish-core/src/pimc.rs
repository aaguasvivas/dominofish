//! PIMC, perfect-information Monte Carlo (M4-02; docs/PLAN.md, D18): the
//! value of each legal move is its mean over worlds drawn from the seat's
//! InfoSet (core/src/sampler.rs), each world solved exactly
//! (core/src/solver.rs). The same worlds serve every move.
//!
//! Inside a world every seat sees every hand, so 2v2 PIMC treats the partner
//! as omniscient, and 1v1 PIMC treats the boneyard order as known.

use crate::infoset::InfoSet;
use crate::rng::Rng;
use crate::rules::{Move, MoveList, Phase, team};
use crate::sampler::{SampleError, Sampler};
use crate::solver::{self, Objective, Table, WIN};

/// The budgets of one analysis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Worlds drawn.
    pub worlds: u32,
    /// The node cap of each solve.
    pub nodes: u64,
    /// The table holds 2^table_bits entries of 16 bytes.
    pub table_bits: u32,
}

/// One legal move's values over the worlds, for the seat's team: in round
/// points, the rest of the round's margin after the move, its own points
/// included; with the win objective (D21), the probability in millionths to
/// win the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MoveValue {
    pub mv: Move,
    /// The sum of the move's values over the worlds.
    pub sum: i64,
    /// The sum of their squares.
    pub sum_squares: i64,
}

impl MoveValue {
    /// The mean value over `worlds` worlds.
    pub fn mean(&self, worlds: u32) -> f64 {
        self.sum as f64 / f64::from(worlds)
    }

    /// The standard error of the mean over `worlds` worlds; None below 2.
    pub fn standard_error(&self, worlds: u32) -> Option<f64> {
        if worlds < 2 {
            return None;
        }
        let n = f64::from(worlds);
        let s = self.sum as f64;
        let variance = ((self.sum_squares as f64 - s * s / n) / (n - 1.0)).max(0.0);
        Some((variance / n).sqrt())
    }
}

/// The result of `analyze`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Analysis {
    /// Every legal move, best first; equal sums keep the generator's order.
    pub moves: Vec<MoveValue>,
    /// The chosen move: the first of `moves`.
    pub best: Move,
    /// The number of consistent worlds (M2-03).
    pub info_size: u128,
    /// Worlds drawn and solved.
    pub worlds: u32,
    /// Solver nodes over every solve.
    pub nodes: u64,
    /// Solves that reached the node cap and took the greedy value (D15).
    pub capped: u32,
    /// Worlds that disagree with the InfoSet (`Sampler::agrees`).
    pub inconsistent: u32,
    /// 1 when no world could be drawn: the moves then hold no values and
    /// `best` is the first legal move. A run where it is not 0 is void.
    pub fallback: u32,
}

/// Ranks the legal moves of the seat on turn in `info` by their mean round
/// points over `limits.worlds` worlds drawn with an RNG seeded by `seed`.
pub fn analyze(info: &InfoSet, limits: &Limits, seed: u64) -> Analysis {
    analyze_with(info, limits, Objective::Points, seed)
}

/// `analyze` for an objective (D21): with the win objective, by each move's
/// mean probability to win the game over the same worlds.
pub fn analyze_with(info: &InfoSet, limits: &Limits, objective: Objective, seed: u64) -> Analysis {
    assert!(
        info.phase == Phase::Playing && info.turn == info.seat,
        "analyze needs the seat on turn"
    );
    let mut legal = MoveList::new();
    info.legal_moves(&mut legal);
    let mut moves: Vec<MoveValue> = legal
        .as_slice()
        .iter()
        .map(|&mv| MoveValue {
            mv,
            sum: 0,
            sum_squares: 0,
        })
        .collect();
    let mut out = Analysis {
        moves: Vec::new(),
        best: legal.as_slice()[0],
        info_size: 0,
        worlds: 0,
        nodes: 0,
        capped: 0,
        inconsistent: 0,
        fallback: 0,
    };
    let sampler = match Sampler::new(info) {
        Ok(s) => s,
        Err(SampleError::PendingDraw | SampleError::NoWorld) => {
            out.fallback = 1;
            out.moves = moves;
            return out;
        }
    };
    out.info_size = sampler.count();
    let mut rng = Rng::new(seed);
    let mut table = Table::new(limits.table_bits);
    let sign = if team(info.seat) == 0 { 1 } else { -1 };
    for _ in 0..limits.worlds {
        let mut world = sampler.sample(&mut rng);
        if !Sampler::agrees(info, &world.state) {
            out.inconsistent += 1;
        }
        table.clear();
        let state = &mut world.state;
        for value in &mut moves {
            let before = state.scores;
            let played = state.play(value.mv);
            let gain = match objective {
                Objective::Points => {
                    (i32::from(state.scores[0]) - i32::from(before[0]))
                        - (i32::from(state.scores[1]) - i32::from(before[1]))
                }
                Objective::Win(_) => 0,
            };
            let v = if state.phase == Phase::Playing {
                let s = solver::solve_with(state, Some(&mut table), limits.nodes, objective);
                out.nodes += s.nodes;
                out.capped += u32::from(s.capped);
                gain + s.value
            } else {
                match objective {
                    Objective::Points => gain,
                    Objective::Win(w) => solver::win_value(w, state),
                }
            };
            state.undo(value.mv, played.undo);
            // Team 0's value, seen from the seat's team.
            let v = i64::from(match objective {
                Objective::Points => sign * v,
                Objective::Win(_) if sign == 1 => v,
                Objective::Win(_) => WIN - v,
            });
            value.sum += v;
            value.sum_squares += v * v;
        }
        out.worlds += 1;
    }
    // A stable sort: equal sums keep the generator's order.
    moves.sort_by_key(|m| core::cmp::Reverse(m.sum));
    out.best = moves[0].mv;
    out.moves = moves;
    out
}

//! AIVAT control variates (M5; docs/PLAN.md D19), after Burch, Schmid,
//! Moravcik, and Bowling (AIVAT, AAAI 2018): the outcome of a round minus
//! terms that each have mean 0 given everything played before them, so the
//! corrected outcome keeps the mean and loses the luck those terms measure.
//! A term compares a value after a random event with that value's mean over
//! the event: the deal of a round, a 1v1 draw (the boneyard order nobody
//! sees), and a baseline bot's choice among its tied best moves.
//!
//! The value of a state is the exact solver's (D15): the rest of the round's
//! margin, team 0 minus team 1, with a node cap and a table cleared before
//! every value, so it is a function of the state alone. Any such function
//! keeps every term's mean at 0; a better one removes more luck.
//!
//! Only the referee calls this module: it reads full states.

use crate::rng::Rng;
use crate::rules::{Deal, Move, Phase, State};
use crate::solver::{self, Table};
use crate::tile::Tile;

/// Values states with the solver and counts its work.
pub struct Valuer {
    table: Table,
    cap: u64,
    /// Values computed.
    pub values: u64,
    /// Values that reached the node cap (the solver's greedy value).
    pub capped: u64,
}

/// Team 0's points minus team 1's from `before` to `after`.
fn gain(before: [u16; 2], after: [u16; 2]) -> i32 {
    (i32::from(after[0]) - i32::from(before[0])) - (i32::from(after[1]) - i32::from(before[1]))
}

impl Valuer {
    pub fn new(table_bits: u32, cap: u64) -> Valuer {
        Valuer {
            table: Table::new(table_bits),
            cap,
            values: 0,
            capped: 0,
        }
    }

    /// The rest of the round's margin from `state`, team 0 minus team 1; 0
    /// once the round is over.
    pub fn value(&mut self, state: &State) -> i32 {
        if state.phase != Phase::Playing {
            return 0;
        }
        self.table.clear();
        let mut s = *state;
        let sol = solver::solve(&mut s, Some(&mut self.table), self.cap);
        self.values += 1;
        self.capped += u64::from(sol.capped);
        sol.value
    }

    /// The points `mv` scores plus the value after it.
    pub fn after(&mut self, state: &State, mv: Move) -> i32 {
        let mut s = *state;
        s.play(mv);
        gain(state.scores, s.scores) + self.value(&s)
    }
}

fn mean(xs: impl Iterator<Item = i32>) -> f64 {
    let (sum, n) = xs.fold((0i64, 0u32), |(s, n), x| (s + i64::from(x), n + 1));
    sum as f64 / f64::from(n)
}

/// A baseline bot's choice: the value after `chosen` minus the mean value
/// after the moves of its policy, each equally likely (`Bot::policy`).
pub fn action_term(v: &mut Valuer, state: &State, chosen: Move, policy: &[Move]) -> f64 {
    debug_assert!(policy.contains(&chosen));
    if policy.len() < 2 {
        return 0.0;
    }
    let values: Vec<i32> = policy.iter().map(|&m| v.after(state, m)).collect();
    let i = policy
        .iter()
        .position(|&m| m == chosen)
        .expect("the chosen move is in the policy");
    f64::from(values[i]) - mean(values.into_iter())
}

/// A 1v1 draw by the seat on turn in `state`: the value after the draw from
/// the real boneyard minus the mean value after the same draw from `j`
/// uniform reshuffles of the boneyard's tiles. Valid only when no player
/// sees the boneyard order, which then is uniform given the history.
pub fn draw_term(v: &mut Valuer, state: &State, rng: &mut Rng, j: u32) -> f64 {
    let real = v.after(state, Move::Draw);
    let mut order: Vec<Tile> = state.boneyard().to_vec();
    let baseline: Vec<i32> = (0..j)
        .map(|_| {
            for i in (1..order.len()).rev() {
                order.swap(i, rng.below(i as u32 + 1) as usize);
            }
            v.after(&state.with_boneyard(&order), Move::Draw)
        })
        .collect();
    f64::from(real) - mean(baseline.into_iter())
}

/// The deal of a round after round 1: the value at its start minus the mean
/// value at the start of the same round dealt from each of `others`
/// (independent uniform deals). `over` is the previous round's end.
pub fn deal_term(v: &mut Valuer, over: &State, deal: &Deal, others: &[Deal]) -> f64 {
    let real = v.value(&over.next_round(deal));
    let baseline: Vec<i32> = others
        .iter()
        .map(|d| v.value(&over.next_round(d)))
        .collect();
    f64::from(real) - mean(baseline.into_iter())
}

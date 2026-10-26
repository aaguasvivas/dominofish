//! M3: the solver against naive minimax and with the table on and off.
//!
//! Positions: every position of the first two rounds of seeded random games
//! in each mode, the second round led on an empty board. Near-target
//! variants put the same position under scores where a later bonus pays or
//! not (R25). The light test runs in every cargo test; the full check is
//! ignored by default and run by scripts/verify/M3-01.sh in release mode,
//! with SOLVER_GAMES games per mode.

use std::collections::BTreeMap;

use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, MoveList, Phase, State};
use dominofish_core::solver::{self, NO_CAP, Table, tiles_left};

/// Naive minimax stops here; the check records where it finished.
const NAIVE_CAP: u64 = 50_000_000;
/// Targets and scores around the bonus cap: 74 + 25 < 100 pays, 75 + 25 does
/// not; 174 and 175 the same at 200.
const VARIANTS: [(u16, [u16; 2]); 7] = [
    (100, [74, 74]),
    (100, [75, 75]),
    (100, [74, 76]),
    (100, [99, 50]),
    (100, [49, 99]),
    (200, [174, 175]),
    (200, [150, 199]),
];

/// Every position of the first two rounds of game `seed`.
fn positions(mode: Mode, seed: u64) -> Vec<State> {
    let mut rng = Rng::new(seed);
    let mut moves = MoveList::new();
    let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
    let mut out = Vec::new();
    let mut rounds = 1;
    loop {
        match state.phase {
            Phase::Finished => return out,
            Phase::RoundOver if rounds == 2 => return out,
            Phase::RoundOver => {
                state = state.next_round(&Deal::shuffled(&mut rng));
                rounds += 1;
            }
            Phase::Playing => {
                out.push(state);
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                state.play(mv);
            }
        }
    }
}

fn with_scores(state: &State, target: u16, scores: [u16; 2]) -> State {
    let mut s = *state;
    s.target = target;
    s.scores = scores;
    s
}

#[derive(Default)]
struct Tally {
    /// Per tiles left: positions, naive finished.
    by_left: BTreeMap<u32, (u64, u64)>,
    naive: u64,
    variants: u64,
    /// Variants where naive minimax did not finish.
    variants_beyond: u64,
    deeper: u64,
    mismatches: Vec<String>,
}

impl Tally {
    /// The smallest tiles-left count reached, and the largest up to which
    /// naive minimax finished at every position.
    fn naive_range(&self) -> (u32, u32) {
        let low = self.by_left.keys().next().copied().unwrap_or(0);
        let mut high = 0;
        for (&left, &(n, done)) in &self.by_left {
            if n != done {
                break;
            }
            high = left;
        }
        (low, high)
    }
}

/// Compares naive minimax (when it finishes) with the solver with and
/// without the table, at `state` and, up to `variant_left` tiles left, at its
/// near-target variants.
fn check(state: &State, table: &mut Table, variant_left: u32, tally: &mut Tally, tag: &str) {
    let left = tiles_left(state);
    let mut cases = vec![*state];
    if left <= variant_left {
        cases.extend(VARIANTS.iter().map(|&(t, s)| with_scores(state, t, s)));
    }
    for (i, case) in cases.iter().enumerate() {
        let mut s = *case;
        let mut n = 0;
        let naive = solver::minimax(&mut s, NAIVE_CAP, &mut n);
        table.clear();
        let on = solver::solve(&mut s, Some(table), NO_CAP);
        let off = solver::solve(&mut s, None, NO_CAP);
        assert_eq!(s, *case, "the solver must leave the state unchanged");
        if i == 0 {
            let e = tally.by_left.entry(left).or_default();
            e.0 += 1;
            e.1 += u64::from(naive.is_some());
        } else {
            tally.variants += 1;
        }
        if naive.is_none() {
            tally.deeper += 1;
            tally.variants_beyond += u64::from(i > 0);
        } else if i == 0 {
            tally.naive += 1;
        }
        let reference = naive.unwrap_or(off.value);
        if on.value != reference || off.value != reference {
            tally.mismatches.push(format!(
                "{tag} tiles left {left} variant {i}: naive {naive:?} table on {} off {}: {}",
                on.value,
                off.value,
                dominofish_core::position::to_text(case)
            ));
        }
    }
}

fn run(games: u64, max_left: u32, variant_left: u32) -> [(Mode, Tally); 2] {
    let mut table = Table::new(20);
    [Mode::TwoVsTwo, Mode::OneVsOne].map(|mode| {
        let mut tally = Tally::default();
        for g in 0..games {
            let seed = 30_000 + g;
            for state in positions(mode, seed) {
                if tiles_left(&state) <= max_left {
                    check(
                        &state,
                        &mut table,
                        variant_left,
                        &mut tally,
                        &format!("{mode:?} game seed {seed}"),
                    );
                }
            }
        }
        (mode, tally)
    })
}

#[test]
fn solver_equals_naive_minimax_on_short_positions() {
    for (mode, tally) in run(8, 12, 10) {
        assert!(tally.mismatches.is_empty(), "{:#?}", tally.mismatches);
        assert!(
            tally.naive > 40 && tally.variants > 100,
            "{mode:?}: {} {}",
            tally.naive,
            tally.variants
        );
    }
}

#[test]
#[ignore = "run by scripts/verify/M3-01.sh in release mode"]
fn solver_equals_naive_minimax_and_table_off() {
    let games: u64 = std::env::var("SOLVER_GAMES")
        .ok()
        .and_then(|g| g.parse().ok())
        .unwrap_or(30);
    let mut line = String::from("solver:");
    let mut mismatches = Vec::new();
    for (mode, tally) in run(games, 28, 18) {
        line.push_str(&format!(
            " [{} games {games}: naive positions {} (finished at every position from {} to {} tiles left), near-target variants {} ({} beyond naive), deeper positions table on and off {}, mismatches {}]",
            if mode == Mode::OneVsOne { "1v1" } else { "2v2" },
            tally.naive,
            tally.naive_range().0,
            tally.naive_range().1,
            tally.variants,
            tally.variants_beyond,
            tally.deeper,
            tally.mismatches.len()
        ));
        mismatches.extend(tally.mismatches);
    }
    println!("{line}");
    for m in &mismatches {
        println!("ERROR {m}");
    }
    assert!(mismatches.is_empty());
}

/// Past the node cap the solver falls back to `greedy`: its move is legal,
/// the state is unchanged, and its value is that move's points plus the
/// greedy margin after it.
#[test]
fn the_capped_fallback_plays_a_legal_greedy_move() {
    let mut checked = 0;
    for mode in [Mode::TwoVsTwo, Mode::OneVsOne] {
        for state in positions(mode, 40_000).into_iter().take(12) {
            let mut s = state;
            let sol = solver::solve(&mut s, None, 5);
            assert_eq!(s, state);
            if !sol.capped {
                continue;
            }
            assert!(state.is_legal(sol.best), "{:?}", sol.best);
            let played = s.play(sol.best);
            let gain = (i32::from(s.scores[0]) - i32::from(state.scores[0]))
                - (i32::from(s.scores[1]) - i32::from(state.scores[1]));
            let rest = if s.phase == Phase::Playing {
                solver::greedy(&mut s)
            } else {
                0
            };
            s.undo(sol.best, played.undo);
            assert_eq!(sol.value, gain + rest);
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked}");
}

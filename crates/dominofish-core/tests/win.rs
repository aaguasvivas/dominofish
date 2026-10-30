//! M6-02: the solver with the win objective (docs/PLAN.md D21) against
//! naive minimax with the same values, and with the table on and off.
//!
//! W comes from the rounds after the first of seeded bot games (core
//! equity), so it varies with the scores. Positions: the first two rounds
//! of seeded random games, each at its own scores and at near-target
//! variants, where a later bonus pays or not (R25) and a round can end the
//! game. A table serves one round at one start, so each round and variant
//! keeps its own table warm across the round's positions. With WIN_FULL=1
//! (scripts/verify/M6-02.sh, release mode) the check runs deeper and on more
//! games.

mod common;

use dominofish_core::infoset::InfoSet;
use dominofish_core::pimc::{Limits, analyze_with};
use dominofish_core::rules::{Mode, Phase, State, team};
use dominofish_core::sampler::Sampler;
use dominofish_core::solver::{self, NO_CAP, Objective, Table, WIN, tiles_left};

/// Naive minimax stops here; positions where it does not finish are
/// counted, and there the table on and off must still agree.
const NAIVE_CAP: u64 = 20_000_000;

/// (target, scores): the position's own scores first, then variants.
const VARIANTS: [(u16, [u16; 2]); 6] = [
    (100, [74, 74]),
    (100, [75, 75]),
    (100, [99, 50]),
    (100, [49, 99]),
    (200, [174, 175]),
    (200, [150, 199]),
];

fn full() -> bool {
    std::env::var("WIN_FULL").as_deref() == Ok("1")
}

#[test]
fn the_win_objective_equals_naive_minimax() {
    let deepest = if full() { 17 } else { 9 };
    let games = if full() { 20 } else { 2 };
    let (mut positions, mut naive_done, mut deeper, mut ended) = (0u64, 0u64, 0u64, 0u64);
    let mut mismatches = Vec::new();
    for mode in [Mode::TwoVsTwo, Mode::OneVsOne] {
        let tables = [common::bot_equity(mode, 100), common::bot_equity(mode, 200)];
        for g in 0..games {
            let record = common::random_record(mode, 100, 9300 + g, false);
            for (r, round) in record.rounds.iter().enumerate().take(2) {
                let (mut state, _) = record.round_start(r).unwrap();
                let own = (state.target, state.scores);
                // A table per variant, warm across the round's positions.
                let mut warm: Vec<Table> = (0..=VARIANTS.len()).map(|_| Table::new(16)).collect();
                for &mv in &round.moves {
                    if state.phase != Phase::Playing {
                        break;
                    }
                    if tiles_left(&state) <= deepest {
                        let cases = std::iter::once(own).chain(VARIANTS.iter().copied());
                        for (i, (target, scores)) in cases.enumerate() {
                            let mut case: State = state;
                            case.target = target;
                            case.scores = scores;
                            let w = &tables[usize::from(target == 200)];
                            let objective = Objective::Win(w);
                            let mut nodes = 0;
                            let mut s = case;
                            let naive =
                                solver::minimax_with(&mut s, NAIVE_CAP, &mut nodes, objective);
                            let on =
                                solver::solve_with(&mut s, Some(&mut warm[i]), NO_CAP, objective);
                            let off = solver::solve_with(&mut s, None, NO_CAP, objective);
                            assert_eq!(s, case, "the solver must leave the state unchanged");
                            let reference = naive.unwrap_or(off.value);
                            if on.value != reference || off.value != reference {
                                mismatches.push(format!(
                                    "{mode:?} game {g} round {r} variant {i} tiles left {}: naive {naive:?} on {} off {}",
                                    tiles_left(&case),
                                    on.value,
                                    off.value
                                ));
                            }
                            positions += 1;
                            naive_done += u64::from(naive.is_some());
                            deeper += u64::from(naive.is_none());
                            ended += u64::from(on.value == 0 || on.value == solver::WIN);
                        }
                    }
                    state.play(mv);
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatches: {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(5)]
    );
    assert!(
        naive_done > if full() { 2000 } else { 200 },
        "{naive_done} naive solves"
    );
    println!(
        "win objective: {positions} positions and variants, {naive_done} equal to naive minimax with win values, {deeper} past its cap with the table on and off equal; {ended} decided (win value 0 or 1); 0 mismatches"
    );
}

#[test]
fn one_consistent_world_gives_the_exact_win_values() {
    // At decisions with one consistent world, PIMC with the win objective
    // gives each move the solver's exact win value for the seat's team,
    // with standard error 0.
    let limits = Limits {
        worlds: 4,
        nodes: 10_000_000,
        table_bits: 16,
    };
    let mut checked = [0u64; 2];
    for (m, mode) in [Mode::OneVsOne, Mode::TwoVsTwo].into_iter().enumerate() {
        let w = common::bot_equity(mode, 100);
        let objective = Objective::Win(&w);
        for g in 0..60u64 {
            let record = common::random_record(mode, 100, 910_000 + g, false);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for ply in 0..round.moves.len() {
                    let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                    let one = Sampler::new(&info).unwrap().count() == 1;
                    if one && (mode == Mode::TwoVsTwo || info.boneyard_count <= 1) {
                        let a = analyze_with(&info, &limits, objective, 6);
                        for value in &a.moves {
                            let mut s = state;
                            s.play(value.mv);
                            let v = if s.phase == Phase::Playing {
                                solver::solve_with(&mut s, None, NO_CAP, objective).value
                            } else {
                                solver::win_value(&w, &s)
                            };
                            let v = i64::from(if team(info.seat) == 0 { v } else { WIN - v });
                            let n = i64::from(limits.worlds);
                            assert_eq!(
                                (value.sum, value.sum_squares),
                                (n * v, n * v * v),
                                "{mode:?} {:?}",
                                value.mv
                            );
                        }
                        assert_eq!(a.moves[0].standard_error(a.worlds), Some(0.0));
                        checked[m] += 1;
                    }
                    state.play(round.moves[ply]);
                }
            }
        }
    }
    assert!(checked.iter().all(|&n| n >= 30), "{checked:?}");
    println!(
        "win pimc one world: 1v1 {} decisions, 2v2 {} decisions, every move's mean equals its exact win value with standard error 0",
        checked[0], checked[1]
    );
}

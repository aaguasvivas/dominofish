//! M4-02: PIMC analysis (core/src/pimc.rs). With PIMC_FULL=1
//! (scripts/verify/M4-02.sh, release mode) the sweep plays 20 games per
//! mode; without it, 2, so check.sh stays fast.

mod common;

use dominofish_core::infoset::InfoSet;
use dominofish_core::pimc::{Analysis, Limits, analyze};
use dominofish_core::record::GameRecord;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Mode, Move, MoveList, Phase, State, team};
use dominofish_core::sampler::Sampler;
use dominofish_core::solver::{self, NO_CAP};

const LIMITS: Limits = Limits {
    worlds: 4,
    nodes: 10_000_000,
    table_bits: 16,
};

fn full() -> bool {
    std::env::var("PIMC_FULL").as_deref() == Ok("1")
}

/// The value of `mv` in `state` for the seat's team: its points plus the
/// rest of the round under perfect play, solved without a table.
fn exact(state: &State, mv: Move, seat: u8) -> i64 {
    let mut s = *state;
    let before = s.scores;
    s.play(mv);
    let gain = (i32::from(s.scores[0]) - i32::from(before[0]))
        - (i32::from(s.scores[1]) - i32::from(before[1]));
    let v = if s.phase == Phase::Playing {
        gain + solver::solve(&mut s, None, NO_CAP).value
    } else {
        gain
    };
    i64::from(if team(seat) == 0 { v } else { -v })
}

/// Every decision of the seat on turn in seeded games, with the true state.
fn each_decision(
    mode: Mode,
    games: u64,
    seed: u64,
    mut f: impl FnMut(&GameRecord, &State, &InfoSet),
) {
    for g in 0..games {
        let record = common::random_record(mode, 100, seed + g, false);
        for (r, round) in record.rounds.iter().enumerate() {
            let (mut state, _) = record.round_start(r).unwrap();
            for ply in 0..round.moves.len() {
                let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                f(&record, &state, &info);
                state.play(round.moves[ply]);
            }
        }
    }
}

fn value_of(a: &Analysis, mv: Move) -> (i64, i64) {
    let m = a
        .moves
        .iter()
        .find(|m| m.mv == mv)
        .expect("every legal move is ranked");
    (m.sum, m.sum_squares)
}

#[test]
fn the_same_seed_gives_the_same_analysis() {
    let mut checked = 0;
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        each_decision(mode, 1, 900_000, |_, state, info| {
            if solver::tiles_left(state) > 18 {
                return;
            }
            let a = analyze(info, &LIMITS, 5);
            assert_eq!(a, analyze(info, &LIMITS, 5));
            checked += 1;
        });
    }
    assert!(checked > 20);
    println!("pimc determinism: {checked} decisions, equal analyses from equal seeds");
}

#[test]
fn one_consistent_world_gives_the_exact_values() {
    let mut checked = [0u64; 2];
    for (m, mode) in [Mode::OneVsOne, Mode::TwoVsTwo].into_iter().enumerate() {
        each_decision(mode, 100, 910_000, |_, state, info| {
            let count = Sampler::new(info).unwrap().count();
            if count != 1 || (mode == Mode::OneVsOne && info.boneyard_count > 1) {
                return;
            }
            let a = analyze(info, &LIMITS, 6);
            for mv in a.moves.iter().map(|m| m.mv) {
                let v = exact(state, mv, info.seat);
                let w = i64::from(LIMITS.worlds);
                assert_eq!(value_of(&a, mv), (w * v, w * v * v), "{mode:?} {mv:?}");
            }
            assert_eq!(a.moves[0].standard_error(a.worlds), Some(0.0));
            checked[m] += 1;
        });
    }
    assert!(checked.iter().all(|&n| n >= 50), "{checked:?}");
    println!(
        "pimc one world: 1v1 {} decisions, 2v2 {} decisions, every move's mean equals its exact value with standard error 0",
        checked[0], checked[1]
    );
}

#[test]
fn per_world_values_equal_the_solver() {
    let mut checked = 0u64;
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        each_decision(mode, 3, 920_000, |_, state, info| {
            if solver::tiles_left(state) > 16 {
                return;
            }
            let seed = 7 + checked;
            let a = analyze(info, &LIMITS, seed);
            let sampler = Sampler::new(info).unwrap();
            let mut rng = Rng::new(seed);
            let mut legal = MoveList::new();
            info.legal_moves(&mut legal);
            let mut sums = vec![(0i64, 0i64); legal.len()];
            for _ in 0..LIMITS.worlds {
                let world = sampler.sample(&mut rng);
                for (k, &mv) in legal.as_slice().iter().enumerate() {
                    let v = exact(&world.state, mv, info.seat);
                    sums[k].0 += v;
                    sums[k].1 += v * v;
                }
            }
            for (k, &mv) in legal.as_slice().iter().enumerate() {
                assert_eq!(value_of(&a, mv), sums[k], "{mode:?} {mv:?}");
            }
            checked += 1;
        });
    }
    assert!(checked > 100);
    println!(
        "pimc per world: {checked} decisions, every move's values equal the solver's without a table on the same worlds"
    );
}

#[test]
fn every_decision_is_legal_with_no_inconsistent_world() {
    let games = if full() { 20 } else { 2 };
    let mut parts = Vec::new();
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        let (mut n, mut nodes, mut capped, mut inconsistent, mut fallback) =
            (0u64, 0u64, 0u64, 0u64, 0u64);
        let mut seed = Rng::new(930_000);
        each_decision(mode, games, 930_000, |_, state, info| {
            let a = analyze(info, &LIMITS, seed.next_u64());
            let mut legal = MoveList::new();
            state.legal_moves(&mut legal);
            assert!(
                legal.as_slice().contains(&a.best),
                "{mode:?}: {:?} is not legal",
                a.best
            );
            assert_eq!(a.moves.len(), legal.len());
            assert_eq!(a.worlds, LIMITS.worlds);
            assert!(a.info_size >= 1);
            n += 1;
            nodes += a.nodes;
            capped += u64::from(a.capped);
            inconsistent += u64::from(a.inconsistent);
            fallback += u64::from(a.fallback);
        });
        parts.push(format!(
            "[{mode:?}: games {games} decisions {n} worlds per decision {} nodes {nodes} capped {capped} inconsistent {inconsistent} fallback {fallback}]",
            LIMITS.worlds
        ));
        assert_eq!((inconsistent, fallback), (0, 0), "{mode:?}");
    }
    println!(
        "pimc sweep ({}): {}",
        if full() { "full" } else { "smoke" },
        parts.join(" ")
    );
}

#[test]
fn a_table_does_not_change_the_values() {
    // The table is a cache: the same worlds solved with 2^12 and 2^16
    // entries give the same sums.
    let mut checked = 0;
    each_decision(Mode::TwoVsTwo, 1, 940_000, |_, state, info| {
        if solver::tiles_left(state) > 20 {
            return;
        }
        let small = Limits {
            table_bits: 12,
            ..LIMITS
        };
        let (a, b) = (analyze(info, &LIMITS, 9), analyze(info, &small, 9));
        let sums = |x: &Analysis| {
            let mut v: Vec<(Move, i64, i64)> = x
                .moves
                .iter()
                .map(|m| (m.mv, m.sum, m.sum_squares))
                .collect();
            v.sort_by_key(|&(mv, ..)| format!("{mv:?}"));
            v
        };
        assert_eq!(sums(&a), sums(&b));
        checked += 1;
    });
    assert!(checked > 10);
    println!("pimc tables: {checked} decisions, equal values with 2^12 and 2^16 entries");
}

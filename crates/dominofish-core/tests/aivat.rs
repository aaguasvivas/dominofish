//! M5-01: the AIVAT terms (core/src/aivat.rs) and the bots' exact policies.
//! At decisions of seeded bot games in both modes: each bot's action terms
//! sum to exactly 0 over its policy, and its decisions over many seeds use
//! every move of its policy and no other; a draw term from a boneyard of
//! one tile and a deal term against the real deal are 0; and a value is a
//! function of the state (the same with any table history).

mod common;

use dominofish_core::aivat::{self, Valuer};
use dominofish_core::bots::Bot;
use dominofish_core::infoset::InfoSet;
use dominofish_core::referee::play_bots;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Mode, Move, MoveList, Phase};
use dominofish_core::solver::{self, NO_CAP};

#[test]
fn action_terms_sum_to_zero_over_the_policy() {
    let mut v = Valuer::new(16, 2_000_000);
    let (mut decisions, mut ties) = (0u64, 0u64);
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        for (k, &bot) in Bot::ALL.iter().enumerate() {
            let record = play_bots(mode, 100, [bot; 4], 5000 + k as u64);
            let round = &record.rounds[0];
            let (mut state, _) = record.round_start(0).unwrap();
            for (ply, &mv) in round.moves.iter().enumerate() {
                if state.phase != Phase::Playing {
                    break;
                }
                let info = InfoSet::new(&record, 0, ply, state.turn).unwrap();
                let mut policy = MoveList::new();
                bot.policy(&info, &mut policy);
                assert!(
                    policy.as_slice().contains(&mv),
                    "{bot:?} played outside its policy"
                );
                // Decisions over many seeds: only policy moves, each one.
                let mut seen = MoveList::new();
                for s in 0..64 {
                    let d = bot.decide(&info, &mut Rng::new(s));
                    assert!(policy.as_slice().contains(&d));
                    if !seen.as_slice().contains(&d) {
                        seen.push(d);
                    }
                }
                assert_eq!(
                    seen.len(),
                    policy.len(),
                    "{bot:?} never chose some tied move"
                );
                if policy.len() > 1 && solver::tiles_left(&state) <= 20 {
                    let sum: f64 = policy
                        .as_slice()
                        .iter()
                        .map(|&a| aivat::action_term(&mut v, &state, a, policy.as_slice()))
                        .sum();
                    assert!(sum.abs() < 1e-9, "{bot:?}: terms sum to {sum}");
                    ties += 1;
                }
                decisions += 1;
                state.play(mv);
            }
        }
    }
    assert!(ties > 20, "{ties}");
    println!(
        "aivat actions: {decisions} decisions, each bot's choices cover exactly its policy; {ties} tied decisions whose action terms sum to 0"
    );
}

#[test]
fn deterministic_events_give_zero_terms() {
    let mut v = Valuer::new(16, 2_000_000);
    let mut draws = 0;
    let mut rng = Rng::new(3);
    for seed in 0..200u64 {
        let record = play_bots(Mode::OneVsOne, 100, [Bot::Random; 4], 6000 + seed);
        let (mut state, _) = record.round_start(0).unwrap();
        for &mv in &record.rounds[0].moves {
            if mv == Move::Draw && state.boneyard().len() == 1 {
                assert_eq!(aivat::draw_term(&mut v, &state, &mut rng, 4), 0.0);
                draws += 1;
            }
            state.play(mv);
        }
        if record.rounds.len() > 1 {
            let (over, _) = {
                let (mut s, _) = record.round_start(0).unwrap();
                for &mv in &record.rounds[0].moves {
                    s.play(mv);
                }
                (s, ())
            };
            let deal = record.rounds[1].deal;
            assert_eq!(aivat::deal_term(&mut v, &over, &deal, &[deal, deal]), 0.0);
        }
    }
    assert!(draws > 5, "{draws}");
    println!(
        "aivat zero: {draws} draws from a one-tile boneyard and every round-2 deal against itself give 0"
    );
}

#[test]
fn a_value_is_a_function_of_the_state() {
    let mut v = Valuer::new(16, NO_CAP);
    let mut checked = 0;
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        for seed in 78..84 {
            let record = common::random_record(mode, 100, seed, false);
            let start = record.round_start(0).unwrap().0;
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for &mv in &round.moves {
                    if solver::tiles_left(&state) <= 14 {
                        let a = v.value(&state);
                        // Another state's search in between leaves the table warm.
                        let _ = v.value(&start);
                        let mut s = state;
                        let exact = solver::solve(&mut s, None, NO_CAP).value;
                        assert_eq!((a, v.value(&state)), (exact, exact));
                        checked += 1;
                    }
                    state.play(mv);
                }
            }
        }
    }
    assert!(checked > 100, "{checked}");
    println!(
        "aivat values: {checked} states, equal to the solver without a table, whatever ran before"
    );
}

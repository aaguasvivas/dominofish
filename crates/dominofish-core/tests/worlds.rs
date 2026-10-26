//! M2-03: exact counts of consistent worlds (core/src/worlds.rs). The counts
//! of BUILD.md Leads, the void-count formula, and the worked example; the
//! true world is always among the counted ones; and on positions small
//! enough to enumerate, the count equals a brute-force oracle that builds
//! every candidate deal and keeps it only if replaying the record gives the
//! same public record and the same own hand.

mod common;

use common::oracle::{binomial, brute_force, decisions, multinomial, no_double_deal, true_groups};

use dominofish_core::infoset::InfoSet;
use dominofish_core::record::GameRecord;
use dominofish_core::rules::{Mode, Phase};
use dominofish_core::tile::{ALL, SUIT, Tile};
use dominofish_core::worlds::{Holder, Worlds};

/// The first InfoSet in seeded games that `pick` accepts.
fn find(mode: Mode, pick: impl Fn(&GameRecord, usize, &InfoSet) -> bool) -> (GameRecord, InfoSet) {
    for seed in 0..500 {
        let record = common::random_record(mode, 100, seed, false);
        for r in 0..record.rounds.len() {
            for seat in 0..mode.seats() {
                let info = InfoSet::new(&record, r, 0, seat).unwrap();
                if pick(&record, r, &info) {
                    return (record, info);
                }
            }
        }
    }
    panic!("no such position in 500 games");
}

#[test]
fn counts_from_the_leads() {
    // 2v2 free lead: 21! / (7!)^3.
    let (_, info) = find(Mode::TwoVsTwo, |_, r, i| r > 0 && i.seat == i.starter);
    assert_eq!(Worlds::new(&info).unwrap().count(), 399_072_960);
    // 2v2 round-1 non-opener: 20! / (6! 7! 7!).
    let (_, info) = find(Mode::TwoVsTwo, |_, r, i| r == 0 && i.seat != i.starter);
    assert_eq!(Worlds::new(&info).unwrap().count(), 133_024_320);
    // 1v1 later round: C(21, 7) hands for the other seat.
    let (_, info) = find(Mode::OneVsOne, |_, r, _| r > 0);
    assert_eq!(Worlds::new(&info).unwrap().count(), 116_280);
    // 1v1 round-1 non-opener after the 6-6: C(20, 6) = 38,760; after a
    // lower double d, the higher doubles leave the dealt hand.
    for d in [6u8, 5, 4] {
        let (_, info) = find(Mode::OneVsOne, |_, r, i| {
            r == 0 && i.seat != i.starter && i.opening.map(|(_, t)| t) == Some(Tile::new(d, d))
        });
        let higher = u64::from(6 - d);
        assert_eq!(
            Worlds::new(&info).unwrap().count(),
            binomial(20 - higher, 6),
            "opening {d}-{d}"
        );
    }
    println!(
        "worlds leads: 399072960, 133024320, 116280, and C(20 - h, 6) after a d-d opening (d = 6, 5, 4) reproduced"
    );
}

#[test]
fn void_counts_and_the_worked_example() {
    // A seat void in s numbers can hold only (7-s)(8-s)/2 tiles.
    for subset in 0u8..128 {
        let s = subset.count_ones();
        let void = (0..7)
            .filter(|p| subset & (1 << p) != 0)
            .fold(0u32, |m, p| m | SUIT[p as usize]);
        assert_eq!((ALL & !void).count_ones(), (7 - s) * (8 - s) / 2);
    }
    // The worked example (BUILD.md Leads), counted from an InfoSet through
    // Worlds::new: board 3-4 4-1 1-6 6-5 5-4 (so 4-1, 4-3, 4-5 are down;
    // ends 3 and 4), n holds 4-4 and 4-2, the partner s passed on 4|4 (void
    // in 4s), the opponent e passed on 6|6 (void in 6s). The hidden 4s are
    // 4-0 and 4-6; s holds neither and e cannot hold 4-6, so w holds 4-6 in
    // every world, while 4-0 may be with e or w.
    let text = "v=1 mode=2v2 target=100 scores=0,0 seat=n hand=0-0,1-1,2-2,2-4,4-4 board=4881400 ends=34 turn=n starter=w last=w passes=0 phase=P sizes=5,6,6,6 pile=0 opening=- voids=0,40,10,0 excluded=0,0,0,0 history=-";
    let info = InfoSet::from_text(text).expect("a valid InfoSet text");
    let w = Worlds::new(&info).expect("2v2 worlds");
    let others: Vec<Holder> = w.groups().iter().map(|g| g.0).collect();
    assert_eq!(
        others,
        [
            Holder::Hand { seat: 1 },
            Holder::Hand { seat: 2 },
            Holder::Hand { seat: 3 }
        ]
    );
    let total = w.count();
    let (four_six, four_zero) = (Tile::new(4, 6), Tile::new(4, 0));
    assert!(total > 0);
    assert_eq!(
        w.count_with(four_six, 2),
        total,
        "w holds 4-6 in every world"
    );
    assert_eq!(w.count_with(four_six, 0) + w.count_with(four_six, 1), 0);
    assert_eq!(w.count_with(four_zero, 1), 0, "the partner holds no 4");
    assert!(w.count_with(four_zero, 0) > 0 && w.count_with(four_zero, 2) > 0);
    println!(
        "worlds deductions: (7-s)(8-s)/2 for all 128 void sets; the 4-6 example, counted from its InfoSet, holds in all {total} worlds"
    );
}

#[test]
fn the_true_world_is_always_counted() {
    let mut checked = 0u64;
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        decisions(mode, 40, |record, r, ply, info| {
            let w = Worlds::new(info).expect("a seat's own decision has no pending draw");
            let truth = true_groups(record, r, ply, info.seat);
            for &(t, allowed) in w.tiles() {
                let g = truth
                    .iter()
                    .find(|(x, _)| *x == t)
                    .map(|&(_, g)| g)
                    .unwrap_or_else(|| panic!("{t} has no true group"));
                assert!(
                    allowed & (1 << g) != 0,
                    "{mode:?} round {r} ply {ply}: {t} is really in group {g}, allowed {allowed:b}"
                );
            }
            assert!(w.count() >= 1);
            checked += 1;
        });
    }
    println!("worlds truth: decisions {checked}, the true world allowed at each");
}

#[test]
fn counts_equal_a_brute_force_oracle() {
    let mut per_mode = [0u64; 2];
    let mut worlds = [0u64; 2];
    // Positions where the constraints cut the count below the unconstrained
    // multinomial, and 1v1 positions after a draw of the other seat (where
    // the time-stamped voids apply).
    let mut constrained = [0u64; 2];
    let mut after_draw = 0u64;
    for (m, mode) in [Mode::OneVsOne, Mode::TwoVsTwo].into_iter().enumerate() {
        decisions(mode, 300, |record, r, ply, info| {
            let w = Worlds::new(info).unwrap();
            let drew = w.groups().len() > 2 && mode == Mode::OneVsOne;
            // Keep at most 150 positions per mode, and in 1v1 at most 75
            // without a draw of the other seat.
            if multinomial(&w) > 3000
                || per_mode[m] >= 150
                || (mode == Mode::OneVsOne && !drew && per_mode[m] - after_draw >= 75)
            {
                return;
            }
            let oracle = brute_force(record, r, ply, info, &w);
            assert_eq!(
                w.count(),
                oracle,
                "{mode:?} round {r} ply {ply} seat {}",
                info.seat
            );
            per_mode[m] += 1;
            worlds[m] += oracle;
            constrained[m] += u64::from(oracle < multinomial(&w));
            after_draw += u64::from(drew);
        });
    }
    assert!(per_mode.iter().all(|&n| n >= 50), "{per_mode:?}");
    assert!(
        constrained.iter().all(|&n| n >= 20) && after_draw >= 50,
        "{constrained:?} {after_draw}"
    );
    println!(
        "worlds oracle: 1v1 positions {} ({} after a draw of the other seat, {} constrained) worlds {}; 2v2 positions {} ({} constrained) worlds {}; counts equal the brute force",
        per_mode[0], after_draw, constrained[0], worlds[0], per_mode[1], constrained[1], worlds[1]
    );
}

#[test]
fn no_double_openings_are_counted_exactly() {
    use dominofish_core::record::RoundRecord;
    use dominofish_core::rng::Rng;
    use dominofish_core::rules::{MoveList, State};
    let mut rng = Rng::new(424242);
    let (mut decisions_checked, mut excluded, mut oracle_positions, mut openers) =
        (0u64, 0u64, 0u64, [0u64; 2]);
    for _ in 0..300 {
        let deal = no_double_deal(&mut rng);
        let (mut state, opening) = State::first_round(Mode::OneVsOne, 100, &deal);
        assert!(!opening.is_double());
        openers[state.starter as usize] += 1;
        let mut record = GameRecord {
            mode: Mode::OneVsOne,
            target: 100,
            rounds: vec![RoundRecord {
                deal,
                moves: vec![],
            }],
        };
        let mut moves = MoveList::new();
        while state.phase == Phase::Playing {
            let ply = record.rounds[0].moves.len();
            let info = InfoSet::new(&record, 0, ply, state.turn).unwrap();
            let w = Worlds::new(&info).unwrap();
            let truth = true_groups(&record, 0, ply, info.seat);
            for &(t, allowed) in w.tiles() {
                let g = truth
                    .iter()
                    .find(|(x, _)| *x == t)
                    .map(|&(_, g)| g)
                    .unwrap();
                assert!(
                    allowed & (1 << g) != 0,
                    "no-double round, ply {ply}: {t} is really in group {g}"
                );
            }
            excluded += u64::from(info.dealt_excluded.iter().any(|&m| m != 0));
            if multinomial(&w) <= 3000 && oracle_positions < 60 {
                assert_eq!(
                    w.count(),
                    brute_force(&record, 0, ply, &info, &w),
                    "no-double oracle, ply {ply}"
                );
                oracle_positions += 1;
            }
            decisions_checked += 1;
            state.legal_moves(&mut moves);
            let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
            state.play(mv);
            record.rounds[0].moves.push(mv);
        }
    }
    assert!(
        excluded > 0 && oracle_positions >= 20 && openers.iter().all(|&n| n > 0),
        "{excluded} {oracle_positions} {openers:?}"
    );
    println!(
        "worlds no-double: decisions {decisions_checked} in 300 no-double 1v1 rounds (opener n {} times, s {}), {excluded} with dealt exclusions, true world allowed at each; {oracle_positions} positions equal the brute force",
        openers[0], openers[1]
    );
}

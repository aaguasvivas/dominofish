//! M2-04: the baseline bots. InfoSet::legal_moves equals the referee's set at
//! every decision; every pairing of bots plays full games in both modes at
//! targets 100 and 200 with only legal moves; games replay identically from
//! a seed; and each bot's choice maximizes its documented criterion.

mod common;

use dominofish_core::bots::Bot;
use dominofish_core::infoset::InfoSet;
use dominofish_core::record::GameRecord;
use dominofish_core::referee::play_bots;
use dominofish_core::rules::{Mode, Move, MoveList, Phase};
use dominofish_core::tile::SUIT;

/// Every decision of a recorded game: (InfoSet of the seat on turn, the
/// referee's legal moves, the move made).
fn decisions(record: &GameRecord, mut f: impl FnMut(&InfoSet, &MoveList, Move)) {
    for (r, round) in record.rounds.iter().enumerate() {
        let (mut state, _) = record.round_start(r).unwrap();
        for (ply, &mv) in round.moves.iter().enumerate() {
            if state.phase == Phase::Playing {
                let info = InfoSet::new(record, r, ply, state.turn).unwrap();
                let mut legal = MoveList::new();
                state.legal_moves(&mut legal);
                f(&info, &legal, mv);
            }
            state.play(mv);
        }
    }
}

#[test]
fn infoset_legal_moves_equal_the_referees() {
    let mut checked = 0;
    for seed in 0..60 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            decisions(
                &common::random_record(mode, 100, seed, false),
                |info, legal, _| {
                    let mut mine = MoveList::new();
                    info.legal_moves(&mut mine);
                    assert_eq!(mine.as_slice(), legal.as_slice());
                    checked += 1;
                },
            );
        }
    }
    println!("bots legal: decisions {checked} with InfoSet moves equal to the referee's");
}

#[test]
fn every_pairing_plays_legal_full_games_and_replays_identically() {
    let (mut games, mut moves) = (0, 0);
    for (i, &a) in Bot::ALL.iter().enumerate() {
        for &b in &Bot::ALL[i..] {
            for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
                for target in [100, 200] {
                    for seed in 0..5u64 {
                        let bots = [a, b, a, b];
                        let record = play_bots(mode, target, bots, 1000 + seed);
                        assert_eq!(record, play_bots(mode, target, bots, 1000 + seed), "replay");
                        let text = record.to_text();
                        assert_eq!(
                            GameRecord::from_text(&text).unwrap(),
                            record,
                            "the record is valid"
                        );
                        games += 1;
                        moves += record.rounds.iter().map(|r| r.moves.len()).sum::<usize>();
                    }
                }
            }
        }
    }
    println!(
        "bots games: {games} full games of every pairing in both modes at 100 and 200, {moves} moves, all legal, each replayed identically"
    );
}

#[test]
fn each_bot_maximizes_its_criterion() {
    let mut checked = [0u32; 4];
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        for seed in 0..40u64 {
            for (k, &bot) in Bot::ALL.iter().enumerate() {
                let record = play_bots(mode, 100, [bot; 4], 5000 + seed);
                decisions(&record, |info, legal, mv| {
                    let plays: Vec<_> = legal
                        .as_slice()
                        .iter()
                        .filter_map(|m| match *m {
                            Move::Play { tile, end } => Some((tile, info.ends_after(tile, end))),
                            _ => None,
                        })
                        .collect();
                    let Move::Play { tile, end } = mv else {
                        assert!(
                            plays.is_empty(),
                            "{bot:?} drew or passed with a play available"
                        );
                        return;
                    };
                    let chosen = (tile, info.ends_after(tile, end));
                    match bot {
                        Bot::Random => {}
                        Bot::Heaviest => {
                            assert_eq!(
                                chosen.0.pips(),
                                plays.iter().map(|p| p.0.pips()).max().unwrap()
                            );
                        }
                        Bot::Accompanied => {
                            // The pip it exposes is one it keeps the most of.
                            let kept = |(t, ends): (dominofish_core::tile::Tile, (u8, u8))| {
                                let exposed = if info.board == 0 {
                                    ends.0.max(ends.1)
                                } else if ends.0 != info.left {
                                    ends.0
                                } else {
                                    ends.1
                                };
                                (info.hand & !t.bit() & SUIT[exposed as usize]).count_ones()
                            };
                            assert_eq!(kept(chosen), plays.iter().map(|&p| kept(p)).max().unwrap());
                        }
                        Bot::Rule => {
                            // When a play leaves both ends void for the next
                            // seat and not both void for the partner, it takes
                            // one such play.
                            let next = info.mode.next(info.seat) as usize;
                            let partner = info.mode.next(info.mode.next(info.seat)) as usize;
                            let forcing = |(_, (l, r)): (dominofish_core::tile::Tile, (u8, u8))| {
                                let pips = (1u8 << l) | (1 << r);
                                info.voids[next] & pips == pips
                                    && !(info.mode == Mode::TwoVsTwo
                                        && info.voids[partner] & pips == pips)
                            };
                            if plays.iter().any(|&p| forcing(p)) {
                                assert!(forcing(chosen), "rule bot missed a forcing play");
                                checked[3] += 1;
                            }
                        }
                    }
                    if bot != Bot::Rule {
                        checked[k] += 1;
                    }
                });
            }
        }
    }
    assert!(checked.iter().all(|&n| n > 0), "{checked:?}");
    println!(
        "bots criteria: decisions checked random {} heaviest {} accompanied {} rule (forcing plays) {}",
        checked[0], checked[1], checked[2], checked[3]
    );
}

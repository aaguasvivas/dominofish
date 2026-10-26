//! M2-05: never-peeks for every bot. At decisions of bot games in both
//! modes, the hidden tiles are rearranged (chains of swaps of tiles the seat
//! cannot see) and kept when the public record, the seat's hand, and the
//! scores stay the same. The rearranged record must give a byte-identical
//! InfoSet text and, for every bot, the same decision at a fixed seed.
//! M4-04 adds PIMC: the same decision, move values, and node counts.

mod common;

use dominofish_core::bots::Bot;
use dominofish_core::infoset::{Action, InfoSet};
use dominofish_core::pimc::{Limits, analyze};
use dominofish_core::record::GameRecord;
use dominofish_core::referee::play_bots;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, Move, Phase};
use dominofish_core::solver::tiles_left;
use dominofish_core::tile::{TILE_COUNT, Tile};

/// The public record: round 1's automatic tile, every move as everyone
/// sees it, the seat's hand, and the scores.
type View = (Option<(u8, Tile)>, Vec<(u8, Action)>, u32, [u16; 2]);

fn public_view(record: &GameRecord, r: usize, ply: usize, seat: u8) -> Option<View> {
    let (mut state, opening) = record.round_start(r).ok()?;
    let opening = opening.map(|t| (state.starter, t));
    let mut out = Vec::new();
    for &mv in &record.rounds[r].moves[..ply] {
        if !state.is_legal(mv) {
            return None;
        }
        let (s, pile) = (state.turn, state.boneyard().len());
        state.play(mv);
        out.push((
            s,
            match mv {
                Move::Play { tile, end } => Action::Play { tile, end },
                Move::Draw => Action::Draw {
                    count: (pile - state.boneyard().len()) as u8,
                },
                Move::Pass => Action::Pass,
            },
        ));
    }
    Some((opening, out, state.hands[seat as usize], state.scores))
}

/// Rearranges the hidden tiles of `record` at round `r`, ply `ply` (chains
/// of up to six swaps), keeping only deals with the same public view, and
/// calls `f` with each such record.
fn rearranged(
    record: &GameRecord,
    r: usize,
    ply: usize,
    seat: u8,
    tries: u32,
    rng: &mut Rng,
    mut f: impl FnMut(&GameRecord, &mut Rng),
) {
    let round = &record.rounds[r];
    let view = public_view(record, r, ply, seat).unwrap();
    let hidden = InfoSet::new(record, r, ply, seat).unwrap().hidden();
    for _ in 0..tries {
        let mut tiles = *round.deal.tiles();
        for _ in 0..=rng.below(6) {
            let (i, j) = (
                rng.below(TILE_COUNT as u32) as usize,
                rng.below(TILE_COUNT as u32) as usize,
            );
            if hidden & tiles[i].bit() != 0 && hidden & tiles[j].bit() != 0 {
                tiles.swap(i, j);
            }
        }
        if tiles == *round.deal.tiles() {
            continue;
        }
        let mut other = record.clone();
        other.rounds[r].deal = Deal::from_tiles(tiles).unwrap();
        if public_view(&other, r, ply, seat).as_ref() == Some(&view) {
            f(&other, rng);
        }
    }
}

#[test]
fn pimc_never_peeks() {
    let limits = Limits {
        worlds: 2,
        nodes: 10_000_000,
        table_bits: 16,
    };
    let mut rng = Rng::new(2027);
    let mut compared = [0u64; 2];
    let mut nodes = 0u64;
    for (m, mode) in [Mode::OneVsOne, Mode::TwoVsTwo].into_iter().enumerate() {
        for seed in 0..40u64 {
            let record = play_bots(mode, 100, [Bot::Rule; 4], 9100 + seed);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for ply in 0..round.moves.len() {
                    let seat = state.turn;
                    if tiles_left(&state) <= 14 && rng.below(4) == 0 {
                        let info = InfoSet::new(&record, r, ply, seat).unwrap();
                        let s = rng.next_u64();
                        let a = analyze(&info, &limits, s);
                        rearranged(&record, r, ply, seat, 4, &mut rng, |other, _| {
                            let info2 = InfoSet::new(other, r, ply, seat).unwrap();
                            assert_eq!(info2.to_text(), info.to_text(), "InfoSet bytes differ");
                            let b = analyze(&info2, &limits, s);
                            assert_eq!(
                                a, b,
                                "{mode:?} seed {seed} round {r} ply {ply}: PIMC differs"
                            );
                            compared[m] += 1;
                            nodes += b.nodes;
                        });
                    }
                    state.play(round.moves[ply]);
                }
            }
        }
    }
    assert!(compared.iter().all(|&n| n >= 100), "{compared:?}");
    println!(
        "never-peeks pimc: rearranged worlds with an identical public record, 1v1 {} and 2v2 {}, in rule-bot games; byte-identical InfoSets and the same PIMC decision, move values, and node counts ({nodes} nodes) at a fixed seed",
        compared[0], compared[1]
    );
}

#[test]
fn every_bot_never_peeks() {
    let mut rng = Rng::new(2026);
    let mut rearranged = [0u64; 4];
    let mut texts = 0u64;
    for (k, &bot) in Bot::ALL.iter().enumerate() {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            for seed in 0..30u64 {
                let record = play_bots(mode, 100, [bot; 4], 9000 + seed);
                for (r, round) in record.rounds.iter().enumerate() {
                    let (mut state, _) = record.round_start(r).unwrap();
                    for ply in 0..round.moves.len() {
                        if state.phase != Phase::Playing || rng.below(3) != 0 {
                            state.play(round.moves[ply]);
                            continue;
                        }
                        let seat = state.turn;
                        let info = InfoSet::new(&record, r, ply, seat).unwrap();
                        let text = info.to_text();
                        assert_eq!(
                            InfoSet::from_text(&text).as_ref(),
                            Some(&info),
                            "the text form round-trips"
                        );
                        texts += 1;
                        let view = public_view(&record, r, ply, seat).unwrap();
                        let hidden = info.hidden();
                        for _ in 0..4 {
                            // A chain of up to six swaps of hidden tiles.
                            let mut tiles = *round.deal.tiles();
                            for _ in 0..=rng.below(6) {
                                let (i, j) = (
                                    rng.below(TILE_COUNT as u32) as usize,
                                    rng.below(TILE_COUNT as u32) as usize,
                                );
                                if hidden & tiles[i].bit() != 0 && hidden & tiles[j].bit() != 0 {
                                    tiles.swap(i, j);
                                }
                            }
                            if tiles == *round.deal.tiles() {
                                continue;
                            }
                            let mut other = record.clone();
                            other.rounds[r].deal = Deal::from_tiles(tiles).unwrap();
                            if public_view(&other, r, ply, seat).as_ref() != Some(&view) {
                                continue;
                            }
                            let info2 = InfoSet::new(&other, r, ply, seat).unwrap();
                            assert_eq!(
                                info2.to_text(),
                                text,
                                "{bot:?} seed {seed} round {r} ply {ply}: InfoSet bytes differ"
                            );
                            for &b in &Bot::ALL {
                                let s = rng.next_u64();
                                assert_eq!(
                                    b.decide(&info, &mut Rng::new(s)),
                                    b.decide(&info2, &mut Rng::new(s)),
                                    "{b:?} decided differently"
                                );
                            }
                            rearranged[k] += 1;
                        }
                        state.play(round.moves[ply]);
                    }
                }
            }
        }
    }
    assert!(rearranged.iter().all(|&n| n >= 200), "{rearranged:?}");
    println!(
        "never-peeks: InfoSet texts {texts} round-trip; rearranged worlds with an identical public record in games of random {} heaviest {} accompanied {} rule {}; byte-identical InfoSets and the same decision of every bot at a fixed seed",
        rearranged[0], rearranged[1], rearranged[2], rearranged[3]
    );
}

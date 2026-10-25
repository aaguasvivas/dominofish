//! M2-01: InfoSet over seeded random game records in both modes. Every field
//! matches the true state's public part and the seat's own hand; every
//! deduction (voids, round-1 exclusions) is true of the hidden hands; and two
//! records that differ only in tiles hidden from the seat, with the same
//! public record and own hand, give equal InfoSets.

use dominofish_core::infoset::{Action, InfoSet, NO_END};
use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, Move, MoveList, Phase, State};
use dominofish_core::tile::{SUIT, TILE_COUNT, Tile};

/// A seeded game of random canonical moves, recorded.
fn random_record(mode: Mode, seed: u64) -> GameRecord {
    let mut rng = Rng::new(seed);
    let mut rounds = vec![RoundRecord {
        deal: Deal::shuffled(&mut rng),
        moves: vec![],
    }];
    let (mut state, _) = State::first_round(mode, 100, &rounds[0].deal);
    let mut moves = MoveList::new();
    loop {
        match state.phase {
            Phase::Finished => break,
            Phase::RoundOver => {
                let deal = Deal::shuffled(&mut rng);
                state = state.next_round(&deal);
                rounds.push(RoundRecord {
                    deal,
                    moves: vec![],
                });
            }
            Phase::Playing => {
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                state.play(mv);
                rounds.last_mut().unwrap().moves.push(mv);
            }
        }
    }
    GameRecord {
        mode,
        target: 100,
        rounds,
    }
}

fn void_mask(pips: u8) -> u32 {
    (0..7)
        .filter(|p| pips & (1 << p) != 0)
        .fold(0, |m, p| m | SUIT[p as usize])
}

#[test]
fn infoset_matches_the_public_state_and_its_deductions_hold() {
    let (mut checked, mut voids_seen, mut exclusions_seen) = (0u64, 0u64, 0u64);
    for seed in 0..60 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let record = random_record(mode, seed);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                let dealt = round.deal;
                for ply in 0..=round.moves.len() {
                    for seat in 0..mode.seats() {
                        let info = InfoSet::new(&record, r, ply, seat).unwrap();
                        assert_eq!(info.hand, state.hands[seat as usize], "seed {seed}");
                        assert_eq!(
                            (info.board, info.turn, info.starter),
                            (state.board, state.turn, state.starter)
                        );
                        assert_eq!(
                            (info.last_played_by, info.passes, info.scores),
                            (state.last_played_by, state.passes, state.scores)
                        );
                        assert_eq!(
                            (info.phase, info.target, info.mode),
                            (state.phase, state.target, state.mode)
                        );
                        assert_eq!(info.boneyard_count as usize, state.boneyard().len());
                        if state.board == 0 {
                            assert_eq!((info.left, info.right), (NO_END, NO_END));
                        } else {
                            assert_eq!((info.left, info.right), (state.left, state.right));
                        }
                        for x in 0..4 {
                            assert_eq!(u32::from(info.hand_sizes[x]), state.hands[x].count_ones());
                            assert_eq!(
                                state.hands[x] & void_mask(info.voids[x]),
                                0,
                                "seed {seed}: seat {x} holds a tile of a void pip"
                            );
                            voids_seen += u64::from(info.voids[x] != 0);
                        }
                        if r == 0 {
                            for x in 0..mode.seats() {
                                let hand = dealt.hand(x).iter().fold(0u32, |m, t| m | t.bit());
                                assert_eq!(
                                    hand & info.dealt_excluded[x as usize],
                                    0,
                                    "seed {seed}: seat {x} was dealt an excluded tile"
                                );
                                exclusions_seen += u64::from(info.dealt_excluded[x as usize] != 0);
                            }
                        }
                        assert_eq!(info.history().len(), ply);
                        for (k, pm) in info.history().iter().enumerate() {
                            match (pm.action, round.moves[k]) {
                                (Action::Play { tile, end }, Move::Play { tile: t, end: e }) => {
                                    assert_eq!((tile, end), (t, e))
                                }
                                (Action::Draw { count }, Move::Draw) => assert!(count >= 1),
                                (Action::Pass, Move::Pass) => {}
                                (a, m) => panic!("seed {seed}: history {a:?} for move {m:?}"),
                            }
                        }
                        checked += 1;
                    }
                    if ply < round.moves.len() {
                        state.play(round.moves[ply]);
                    }
                }
            }
        }
    }
    assert!(voids_seen > 0 && exclusions_seen > 0);
    println!(
        "infoset truth: infosets {checked} with voids {voids_seen} and dealt exclusions {exclusions_seen}, all true of the hidden hands"
    );
}

/// The public record of round `r` up to `ply` (every move as everyone sees
/// it) and the seat's hand there, or None if a move is illegal.
/// The scores are public too: a round's points reveal the pips left.
type View = (Vec<(u8, Action)>, u32, Option<(u8, Tile)>, [u16; 2]);

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
    Some((out, state.hands[seat as usize], opening, state.scores))
}

#[test]
fn infoset_ignores_tiles_hidden_from_its_seat() {
    let mut rng = Rng::new(77);
    let (mut tried, mut same_public, mut different) = (0u64, 0u64, 0u64);
    for seed in 100..160 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let record = random_record(mode, seed);
            for (r, round) in record.rounds.iter().enumerate() {
                for _ in 0..20 {
                    let ply = rng.below(round.moves.len() as u32 + 1) as usize;
                    let seat = rng.below(u32::from(mode.seats())) as u8;
                    let info = InfoSet::new(&record, r, ply, seat).unwrap();
                    let view = public_view(&record, r, ply, seat).unwrap();
                    // Swap two deal positions holding tiles hidden from the
                    // seat at this point, then keep the swap only if the
                    // public record and the seat's hand stay the same.
                    let (i, j) = (
                        rng.below(TILE_COUNT as u32) as usize,
                        rng.below(TILE_COUNT as u32) as usize,
                    );
                    let mut tiles = *round.deal.tiles();
                    let hidden = info.hidden();
                    if i == j || hidden & tiles[i].bit() == 0 || hidden & tiles[j].bit() == 0 {
                        continue;
                    }
                    tiles.swap(i, j);
                    tried += 1;
                    let mut other = record.clone();
                    other.rounds[r].deal = Deal::from_tiles(tiles).unwrap();
                    if public_view(&other, r, ply, seat) != Some(view.clone()) {
                        different += 1;
                        continue;
                    }
                    same_public += 1;
                    assert_eq!(
                        InfoSet::new(&other, r, ply, seat).unwrap(),
                        info,
                        "seed {seed} round {r} ply {ply} seat {seat}"
                    );
                }
            }
        }
    }
    assert!(same_public > 100);
    println!(
        "infoset hidden: swaps {tried}, {same_public} with the same public record gave equal infosets, {different} changed the record"
    );
}

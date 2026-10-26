//! The replay oracle of the world tests (M2-03, M4-01): where each tile of a
//! round really is, the deal positions of each world group, and the
//! brute-force enumeration that rebuilds a deal from an assignment and keeps
//! it if the replay gives the same public record, own hand, and scores.

use dominofish_core::infoset::{Action, InfoSet};
use dominofish_core::record::GameRecord;
use dominofish_core::rules::{Deal, Mode, Move, Phase};
use dominofish_core::tile::{ALL, TILE_COUNT, Tile, tiles};
use dominofish_core::worlds::Worlds;

pub fn binomial(n: u64, k: u64) -> u64 {
    (0..k).fold(1, |acc, i| acc * (n - i) / (i + 1))
}

/// Where each tile of the round really is at the InfoSet's point, in the
/// Worlds group numbering (2v2: the other hands in seat order; 1v1: the
/// other seat's dealt hand, its draws in order, then the boneyard).
pub fn true_groups(record: &GameRecord, r: usize, ply: usize, seat: u8) -> Vec<(Tile, usize)> {
    let deal = record.rounds[r].deal;
    let moves = &record.rounds[r].moves;
    let (mut state, _) = record.round_start(r).unwrap();
    let mut out = Vec::new();
    if record.mode == Mode::TwoVsTwo {
        for &mv in &moves[..ply] {
            state.play(mv);
        }
        for (g, x) in (0..4u8).filter(|&x| x != seat).enumerate() {
            out.extend(tiles(state.hands[x as usize]).map(|t| (t, g)));
        }
        return out;
    }
    let other = 1 - seat;
    let mut batch_of = [usize::MAX; TILE_COUNT];
    for t in deal.hand(other) {
        batch_of[t.index()] = 0;
    }
    let mut batches = 1;
    for (k, &mv) in moves[..ply].iter().enumerate() {
        let mover = state.turn;
        let pile = state.boneyard().to_vec();
        state.play(mv);
        if mv == Move::Draw && mover == other {
            let drawn: Vec<Tile> = pile[state.boneyard().len()..]
                .iter()
                .rev()
                .copied()
                .collect();
            let fit = matches!(moves.get(k + 1), Some(Move::Play { .. }));
            for t in &drawn[..drawn.len() - usize::from(fit)] {
                batch_of[t.index()] = batches;
            }
            batches += 1;
        }
    }
    out.extend(state.boneyard().iter().map(|&t| (t, batches)));
    out.extend(
        tiles(ALL)
            .filter(|t| batch_of[t.index()] != usize::MAX)
            .map(|t| (t, batch_of[t.index()])),
    );
    out
}

/// Decision points of the seat on turn in seeded games.
pub fn decisions(mode: Mode, games: u64, mut f: impl FnMut(&GameRecord, usize, usize, &InfoSet)) {
    for seed in 0..games {
        let record = super::random_record(mode, 100, 1000 + seed, true);
        for (r, round) in record.rounds.iter().enumerate() {
            let (mut state, _) = record.round_start(r).unwrap();
            for ply in 0..round.moves.len() {
                if state.phase == Phase::Playing {
                    let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                    f(&record, r, ply, &info);
                }
                state.play(round.moves[ply]);
            }
        }
    }
}

/// The brute-force oracle: every assignment of the world tiles to groups of
/// the right sizes, built into a deal and kept if the replay gives the same
/// public record, own hand, and scores.
pub fn brute_force(record: &GameRecord, r: usize, ply: usize, info: &InfoSet, w: &Worlds) -> u64 {
    let seat = info.seat;
    let deal = record.rounds[r].deal;
    let want = public_view(record, r, ply, seat).unwrap();
    // Deal positions of each group, in the order a deal fills them.
    let positions = group_positions(record, r, ply, seat);
    let list: Vec<Tile> = w.tiles().iter().map(|&(t, _)| t).collect();
    let mut sizes: Vec<u8> = w.groups().iter().map(|&(_, n)| n).collect();
    let mut chosen: Vec<Vec<Tile>> = vec![vec![]; sizes.len()];
    let mut consistent = 0u64;
    fn rec(
        i: usize,
        list: &[Tile],
        sizes: &mut [u8],
        chosen: &mut Vec<Vec<Tile>>,
        on_full: &mut dyn FnMut(&[Vec<Tile>]),
    ) {
        if i == list.len() {
            on_full(chosen);
            return;
        }
        for g in 0..sizes.len() {
            if sizes[g] > 0 {
                sizes[g] -= 1;
                chosen[g].push(list[i]);
                rec(i + 1, list, sizes, chosen, on_full);
                chosen[g].pop();
                sizes[g] += 1;
            }
        }
    }
    let mut on_full = |chosen: &[Vec<Tile>]| {
        let mut t = *deal.tiles();
        for (g, tiles_of_group) in chosen.iter().enumerate() {
            for (k, &tile) in tiles_of_group.iter().enumerate() {
                t[positions[g][k]] = tile;
            }
        }
        let Some(new_deal) = Deal::from_tiles(t) else {
            return;
        };
        let mut other = record.clone();
        other.rounds[r].deal = new_deal;
        if public_view(&other, r, ply, seat).as_ref() == Some(&want) {
            consistent += 1;
        }
    };
    rec(0, &list, &mut sizes, &mut chosen, &mut on_full);
    consistent
}

/// For each group, its deal positions in fill order. 2v2: an other seat's
/// hand positions not holding tiles it played. 1v1: the dealt hand (its
/// automatic tile's position first), each draw's non-fitting positions in
/// pop order, and the boneyard positions left. Pinned tiles (played 2v2
/// tiles, a draw's fitting tile, the seat's own tiles) keep their positions.
pub fn group_positions(record: &GameRecord, r: usize, ply: usize, seat: u8) -> Vec<Vec<usize>> {
    let deal = record.rounds[r].deal;
    let moves = &record.rounds[r].moves;
    let (mut state, opening) = record.round_start(r).unwrap();
    let mut out = Vec::new();
    if record.mode == Mode::TwoVsTwo {
        let mut played = [0u32; 4];
        played[state.starter as usize] |= state.board;
        for &mv in &moves[..ply] {
            if let Move::Play { tile, .. } = mv {
                played[state.turn as usize] |= tile.bit();
            }
            state.play(mv);
        }
        for x in (0..4usize).filter(|&x| x != seat as usize) {
            out.push(
                (7 * x..7 * x + 7)
                    .filter(|&p| played[x] & deal.tiles()[p].bit() == 0)
                    .collect(),
            );
        }
        return out;
    }
    let other = 1 - seat;
    let mut dealt: Vec<usize> = (7 * other as usize..7 * other as usize + 7).collect();
    if let Some(t) = opening
        && state.starter == other
    {
        let p = deal.tiles().iter().position(|&x| x == t).unwrap();
        dealt.retain(|&x| x != p);
        dealt.insert(0, p);
    }
    out.push(dealt);
    for (k, &mv) in moves[..ply].iter().enumerate() {
        let mover = state.turn;
        let before = state.boneyard().len();
        state.play(mv);
        if mv == Move::Draw && mover == other {
            let popped: Vec<usize> = (state.boneyard().len()..before)
                .rev()
                .map(|i| 14 + i)
                .collect();
            let fit = matches!(moves.get(k + 1), Some(Move::Play { .. }));
            out.push(popped[..popped.len() - usize::from(fit)].to_vec());
        }
    }
    out.push((0..state.boneyard().len()).map(|i| 14 + i).collect());
    out
}

pub type View = (Vec<(u8, Action)>, u32, [u16; 2]);

pub fn public_view(record: &GameRecord, r: usize, ply: usize, seat: u8) -> Option<View> {
    let (mut state, _) = record.round_start(r).ok()?;
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
    Some((out, state.hands[seat as usize], state.scores))
}

pub fn multinomial(w: &Worlds) -> u64 {
    let mut left = w.tiles().len() as u64;
    let mut total = 1u64;
    for &(_, n) in w.groups() {
        total = total.saturating_mul(binomial(left, u64::from(n)));
        left -= u64::from(n);
    }
    total
}

/// A 1v1 deal with no double in either hand: the seven doubles and seven
/// other tiles fill the boneyard, so round 1 opens with a pip-sum tile.
pub fn no_double_deal(rng: &mut dominofish_core::rng::Rng) -> Deal {
    let mut others: Vec<Tile> = tiles(ALL).filter(|t| !t.is_double()).collect();
    for i in (1..others.len()).rev() {
        others.swap(i, rng.below(i as u32 + 1) as usize);
    }
    let mut pile: Vec<Tile> = others.split_off(14);
    pile.extend((0..=6).map(|d| Tile::new(d, d)));
    for i in (1..pile.len()).rev() {
        pile.swap(i, rng.below(i as u32 + 1) as usize);
    }
    others.extend(pile);
    Deal::from_tiles(others.try_into().unwrap()).unwrap()
}

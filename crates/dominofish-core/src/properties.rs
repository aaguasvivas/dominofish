//! M1-08 property tests over states reached by seeded random playouts in
//! both modes (docs/PLAN.md, D4). Each test prints one summary line with its
//! counts; scripts/verify/M1-08.sh reads them.

use crate::perft::lines;
use crate::rng::Rng;
use crate::rules::{Deal, End, Event, Mode, Move, MoveList, NO_SEAT, Phase, State};
use crate::tile::{ALL, SUIT, TILE_COUNT, Tile, mask_pips};

const GAMES: u64 = 1000;

/// Every state of a seeded random game (the start of each round included),
/// with the move made there, through `visit`.
fn playout(mode: Mode, seed: u64, mut visit: impl FnMut(&State, Option<Move>, Option<Event>)) {
    let mut rng = Rng::new(seed);
    let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
    let mut moves = MoveList::new();
    visit(&state, None, None);
    loop {
        match state.phase {
            Phase::Finished => return,
            Phase::RoundOver => {
                state = state.next_round(&Deal::shuffled(&mut rng));
                visit(&state, None, None);
            }
            Phase::Playing => {
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                let event = state.play(mv).event;
                visit(&state, Some(mv), event);
            }
        }
    }
}

fn locked(s: &State) -> bool {
    s.board != 0 && ALL & !s.board & (SUIT[s.left as usize] | SUIT[s.right as usize]) == 0
}

#[test]
fn tranque_invariant_and_geometry() {
    let (mut states, mut locks, mut min_tiles, mut min_margin) = (0u64, 0u64, u32::MAX, i64::MAX);
    for seed in 0..GAMES {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            playout(mode, seed, |s, _, event| {
                states += 1;
                let k = s.left;
                let suit_on_board = s.board != 0
                    && s.left == s.right
                    && (s.board & SUIT[k as usize]).count_ones() == 7;
                assert_eq!(
                    locked(s),
                    suit_on_board,
                    "seed {seed}: locked iff both ends show k and all 7 k tiles are on the board"
                );
                if let Some(Event::Tranque { .. }) = event {
                    assert!(locked(s), "seed {seed}: a tranque leaves the board locked");
                }
                if locked(s) {
                    locks += 1;
                    let n = s.board.count_ones();
                    let margin = i64::from(mask_pips(s.board)) - (6 * i64::from(k) + 42);
                    assert!(
                        n >= 10 && margin >= 0,
                        "seed {seed}: lock with {n} tiles, pips margin {margin}"
                    );
                    min_tiles = min_tiles.min(n);
                    min_margin = min_margin.min(margin);
                }
            });
        }
    }
    assert!(locks > 0);
    println!(
        "property tranque: states {states} locks {locks} fewest board tiles {min_tiles} (target 10) least pips margin over 6k+42 {min_margin} (target 0)"
    );
}

#[test]
fn tile_conservation() {
    let mut states = 0u64;
    for seed in 0..GAMES {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            playout(mode, seed, |s, _, _| {
                states += 1;
                let pile = s.boneyard().iter().fold(0, |m, t| m | t.bit());
                let mut seen = s.board | pile;
                let mut count = s.board.count_ones() + pile.count_ones();
                for h in &s.hands {
                    assert_eq!(seen & h, 0, "seed {seed}: a tile in two places");
                    seen |= h;
                    count += h.count_ones();
                }
                assert_eq!((seen, count), (ALL, TILE_COUNT as u32), "seed {seed}");
            });
        }
    }
    println!("property conservation: states {states}");
}

/// The mirror image: the two ends swapped.
fn mirrored(s: &State) -> State {
    let mut m = *s;
    (m.left, m.right) = (s.right, s.left);
    m.rehashed(s.boneyard())
}

/// The move that makes the mirror image: always the other end, which on an
/// empty board is the other orientation (docs/RULES.md R10).
fn mirror_move(mv: Move) -> Move {
    match mv {
        Move::Play { tile, end } => Move::Play {
            tile,
            end: if end == End::Left {
                End::Right
            } else {
                End::Left
            },
        },
        other => other,
    }
}

/// Samples every `every`-th playing state of the seeded games.
fn sampled(every: u64, mut f: impl FnMut(&State, u64)) -> u64 {
    let mut n = 0u64;
    let mut k = 0u64;
    for seed in 0..GAMES / 4 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            playout(mode, seed, |s, _, _| {
                k += 1;
                if s.phase == Phase::Playing && k.is_multiple_of(every) {
                    f(s, seed);
                    n += 1;
                }
            });
        }
    }
    n
}

#[test]
fn end_mirror_symmetry() {
    let checked = sampled(7, |s, seed| {
        let m = mirrored(s);
        assert_eq!(
            lines(&mut s.clone(), 4),
            lines(&mut m.clone(), 4),
            "seed {seed}: line counts"
        );
        let mut moves = MoveList::new();
        s.legal_moves(&mut moves);
        for &mv in moves.as_slice() {
            let (mut a, mut b) = (*s, m);
            let ea = a.play(mv).event;
            let mm = mirror_move(mv);
            assert!(m.is_legal(mm), "seed {seed}: mirrored move is legal");
            let eb = b.play(mm).event;
            assert_eq!(ea, eb, "seed {seed}: events");
            assert_eq!(mirrored(&a), b, "seed {seed}: results are mirror images");
        }
    });
    println!("property mirror: states {checked} (lines to depth 4 and every move's result)");
}

/// Seats shifted by one place (n to e and so on in 2v2, n to s in 1v1), so
/// the teams trade places and the scores swap with them.
fn rotated(s: &State) -> State {
    let n = s.mode.seats();
    let r = |seat: u8| {
        if seat == NO_SEAT {
            NO_SEAT
        } else {
            (seat + 1) % n
        }
    };
    let mut t = *s;
    for seat in 0..n {
        t.hands[r(seat) as usize] = s.hands[seat as usize];
    }
    t.turn = r(s.turn);
    t.starter = r(s.starter);
    t.last_played_by = r(s.last_played_by);
    t.round_winner = r(s.round_winner);
    t.scores = [s.scores[1], s.scores[0]];
    t.rehashed(s.boneyard())
}

fn rotate_event(e: Option<Event>, n: u8) -> Option<Event> {
    let r = |seat: u8| (seat + 1) % n;
    e.map(|e| match e {
        Event::PaseSalida { team } => Event::PaseSalida { team: 1 - team },
        Event::PaseCorrido { team } => Event::PaseCorrido { team: 1 - team },
        Event::Domino { seat, points } => Event::Domino {
            seat: r(seat),
            points,
        },
        Event::Capicua { seat, points } => Event::Capicua {
            seat: r(seat),
            points,
        },
        Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        } => Event::Tranque {
            blocker: r(blocker),
            rival: r(rival),
            blocker_pips,
            rival_pips,
            winner: r(winner),
            points,
        },
    })
}

#[test]
fn seat_rotation_with_opener_fixed() {
    // The opener and its automatic tile are already decided in every sampled
    // state, so the round-1 tie-break by seat order does not enter.
    let checked = sampled(7, |s, seed| {
        let t = rotated(s);
        assert_eq!(
            lines(&mut s.clone(), 4),
            lines(&mut t.clone(), 4),
            "seed {seed}: line counts"
        );
        let mut moves = MoveList::new();
        s.legal_moves(&mut moves);
        for &mv in moves.as_slice() {
            let (mut a, mut b) = (*s, t);
            let ea = a.play(mv).event;
            let eb = b.play(mv).event;
            assert_eq!(
                rotate_event(ea, s.mode.seats()),
                eb,
                "seed {seed}: events rotate"
            );
            assert_eq!(rotated(&a), b, "seed {seed}: results rotate");
        }
    });
    println!("property rotation: states {checked} (lines to depth 4 and every move's result)");
}

/// The deal with every pip p replaced by perm[p].
fn relabel_deal(deal: &Deal, perm: &[u8; 7]) -> Deal {
    let t = deal
        .tiles()
        .map(|x| Tile::new(perm[x.low() as usize], perm[x.high() as usize]));
    Deal::from_tiles(t).expect("a relabeled deal is a deal")
}

fn relabel_move(mv: Move, perm: &[u8; 7], before: &State) -> Move {
    match mv {
        Move::Play { tile, end } => {
            let t = Tile::new(perm[tile.low() as usize], perm[tile.high() as usize]);
            // A lead's orientation is relative to the tile's low pip, which
            // relabeling can swap.
            let end = if before.board == 0 && perm[tile.low() as usize] > perm[tile.high() as usize]
            {
                if end == End::Left {
                    End::Right
                } else {
                    End::Left
                }
            } else {
                end
            };
            Move::Play { tile: t, end }
        }
        other => other,
    }
}

#[test]
fn line_counts_unchanged_under_pip_relabeling() {
    // Relabeling changes scores and, in round 1, the opener, so it is checked
    // on later rounds (the leader fixed) and only on line counts.
    let mut rng = Rng::new(42);
    let (mut deals, mut late) = (0u64, 0u64);
    for seed in 0..200u64 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let deal = Deal::shuffled(&mut rng);
            let mut perm = [0u8, 1, 2, 3, 4, 5, 6];
            for i in (1..7).rev() {
                perm.swap(i, rng.below(i as u32 + 1) as usize);
            }
            let leader = (seed % u64::from(mode.seats())) as u8;
            let mut a = State::dealt(mode, 100, [0, 0], &deal, leader);
            let mut b = State::dealt(mode, 100, [0, 0], &relabel_deal(&deal, &perm), leader);
            assert_eq!(
                lines(&mut a.clone(), 5),
                lines(&mut b.clone(), 5),
                "seed {seed}: depth 5"
            );
            deals += 1;
            // Walk both to a late position with the same moves, then count
            // every line to the end of the round.
            let mut moves = MoveList::new();
            // In 1v1 the boneyard must be nearly empty too, or the draws make
            // the complete tree too large to count.
            while a.phase == Phase::Playing
                && (a.hands.iter().map(|h| h.count_ones()).sum::<u32>() > 12
                    || a.boneyard().len() > 4)
            {
                a.legal_moves(&mut moves);
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                let rm = relabel_move(mv, &perm, &a);
                assert!(b.is_legal(rm), "seed {seed}: relabeled move is legal");
                a.play(mv);
                b.play(rm);
            }
            if a.phase == Phase::Playing {
                assert_eq!(
                    lines(&mut a.clone(), u32::MAX),
                    lines(&mut b.clone(), u32::MAX),
                    "seed {seed}: complete lines"
                );
                late += 1;
            }
        }
    }
    println!(
        "property relabel: deals {deals} at depth 5, late positions {late} to the end of the round"
    );
}

#[test]
fn determinism() {
    let mut games = 0u64;
    for seed in 0..GAMES / 4 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let mut first = Vec::new();
            playout(mode, seed, |s, mv, e| first.push((s.hash(), mv, e)));
            let mut k = 0;
            playout(mode, seed, |s, mv, e| {
                assert_eq!(first[k], (s.hash(), mv, e), "seed {seed}: step {k}");
                k += 1;
            });
            assert_eq!(k, first.len());
            games += 1;
        }
    }
    println!("property determinism: games {games} replayed identically");
}

#[test]
fn play_undo_round_trip_and_incremental_hash() {
    let mut moves_checked = 0u64;
    for seed in 0..GAMES {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let mut rng = Rng::new(seed ^ 0x5eed);
            let (mut s, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
            let mut moves = MoveList::new();
            while s.phase != Phase::Finished {
                if s.phase == Phase::RoundOver {
                    s = s.next_round(&Deal::shuffled(&mut rng));
                    continue;
                }
                s.legal_moves(&mut moves);
                for &mv in moves.as_slice() {
                    let before = s;
                    let played = s.play(mv);
                    assert_eq!(s.hash(), s.recomputed_hash(), "seed {seed}");
                    s.undo(mv, played.undo);
                    assert_eq!(s, before, "seed {seed}: undo of {mv:?}");
                    moves_checked += 1;
                }
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                s.play(mv);
            }
        }
    }
    println!(
        "property play-undo: moves {moves_checked} undone exactly with the hash equal to recomputed"
    );
}

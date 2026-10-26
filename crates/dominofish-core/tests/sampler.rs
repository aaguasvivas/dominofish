//! M4-01: the world sampler (core/src/sampler.rs), by test T3 of
//! docs/PLAN.md. With T3_FULL=1 (scripts/verify/M4-01.sh, release mode) the
//! tests run T3 as registered and assert its pass rules; without it they run
//! a few games and assert only consistency, so check.sh stays fast.
//!
//! (a) On information sets small enough to enumerate, a chi-square test of
//! the sampled worlds against every consistent world, each with its weight.
//! (b) The true world appears at the uniform rate.
//! (c) Every sampled world, rebuilt into a deal, replays the round's public
//! record, the seat's hand, and the scores, and its state equals the
//! replayed one.

mod common;

use std::collections::BTreeMap;

use common::oracle::{decisions, group_positions, multinomial, no_double_deal, true_groups};
use dominofish_core::infoset::{Action, InfoSet};
use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, Mode, Move, MoveList, Phase, State};
use dominofish_core::sampler::{Sampler, World};
use dominofish_core::tile::{Tile, tiles};
use dominofish_core::worlds::Worlds;

fn full() -> bool {
    std::env::var("T3_FULL").as_deref() == Ok("1")
}

/// One round up to a decision, replayable from any deal of that round.
struct Point {
    mode: Mode,
    target: u16,
    /// Round 1 (its opener and automatic tile come from the deal).
    first: bool,
    start_scores: [u16; 2],
    leader: u8,
    opening: Option<Tile>,
    moves: Vec<Move>,
    actions: Vec<(u8, Action)>,
    seat: u8,
    hand: u32,
    scores: [u16; 2],
    true_deal: Deal,
    positions: Vec<Vec<usize>>,
}

impl Point {
    fn new(record: &GameRecord, r: usize, ply: usize, seat: u8) -> Point {
        let (start, opening) = record.round_start(r).unwrap();
        let moves = record.rounds[r].moves[..ply].to_vec();
        let mut p = Point {
            mode: record.mode,
            target: record.target,
            first: r == 0,
            start_scores: start.scores,
            leader: start.starter,
            opening,
            moves,
            actions: Vec::new(),
            seat,
            hand: 0,
            scores: [0, 0],
            true_deal: record.rounds[r].deal,
            positions: group_positions(record, r, ply, seat),
        };
        let mut state = start;
        for &mv in &p.moves {
            p.actions.push(action(&mut state, mv));
        }
        p.hand = state.hands[seat as usize];
        p.scores = state.scores;
        p
    }

    fn start(&self, deal: &Deal) -> Option<State> {
        if self.first {
            let (s, t) = State::first_round(self.mode, self.target, deal);
            (Some(t) == self.opening).then_some(s)
        } else {
            Some(State::dealt(
                self.mode,
                self.target,
                self.start_scores,
                deal,
                self.leader,
            ))
        }
    }

    /// The state reached by replaying the round from `deal`, if the replay
    /// gives the same opening, public record, own hand, and scores.
    fn replay(&self, deal: &Deal) -> Option<State> {
        let mut state = self.start(deal)?;
        for (k, &mv) in self.moves.iter().enumerate() {
            if !state.is_legal(mv) || action(&mut state, mv) != self.actions[k] {
                return None;
            }
        }
        (state.hands[self.seat as usize] == self.hand && state.scores == self.scores)
            .then_some(state)
    }

    /// The deal with each group's tiles in that group's positions. 1v1: the
    /// dealt hand in ascending positions with the other seat's opening tile
    /// first, and the boneyard in `pile` order when given.
    fn deal_of(&self, groups: &[u32], pile: Option<&[Tile]>) -> Option<Deal> {
        let mut t = *self.true_deal.tiles();
        let other_opening = if self.mode == Mode::OneVsOne && self.first {
            let (s, tile) = State::first_round(self.mode, self.target, &self.true_deal);
            (s.starter != self.seat).then_some(tile)
        } else {
            None
        };
        let last = self.positions.len() - 1;
        for (g, &mask) in groups.iter().enumerate() {
            let mut pos = self.positions[g].clone();
            let mut list: Vec<Tile> = tiles(mask).collect();
            if self.mode == Mode::OneVsOne && g == 0 {
                pos.sort_unstable();
                if let Some(o) = other_opening {
                    list.retain(|&x| x != o);
                    list.insert(0, o);
                }
            }
            if self.mode == Mode::OneVsOne
                && g == last
                && let Some(p) = pile
            {
                list = p.to_vec();
            }
            if list.len() != pos.len() {
                return None;
            }
            for (k, &tile) in list.iter().enumerate() {
                t[pos[k]] = tile;
            }
        }
        Deal::from_tiles(t)
    }

    /// The orders of the other seat's dealt tiles (of 5,040) under which
    /// round 1 opens as it did, when the other 1v1 seat opened with no
    /// double; 1 otherwise.
    fn weight(&self, deal: &Deal) -> u64 {
        let Some(opening) = self.opening else {
            return 1;
        };
        let (s, _) = State::first_round(self.mode, self.target, deal);
        if self.mode != Mode::OneVsOne || opening.is_double() || s.starter == self.seat {
            return 1;
        }
        let other = 1 - self.seat;
        let base = 7 * other as usize;
        let mut hand: Vec<Tile> = deal.hand(other).to_vec();
        let mut kept = 0;
        permutations(&mut hand, 0, &mut |order| {
            let mut t = *deal.tiles();
            t[base..base + 7].copy_from_slice(order);
            let d = Deal::from_tiles(t).unwrap();
            let (s, tile) = State::first_round(self.mode, self.target, &d);
            kept += u64::from(s.starter == other && tile == opening);
        });
        kept
    }
}

fn permutations(v: &mut [Tile], k: usize, f: &mut dyn FnMut(&[Tile])) {
    if k == v.len() {
        f(v);
        return;
    }
    for i in k..v.len() {
        v.swap(k, i);
        permutations(v, k + 1, f);
        v.swap(k, i);
    }
}

fn action(state: &mut State, mv: Move) -> (u8, Action) {
    let (s, pile) = (state.turn, state.boneyard().len());
    state.play(mv);
    let a = match mv {
        Move::Play { tile, end } => Action::Play { tile, end },
        Move::Draw => Action::Draw {
            count: (pile - state.boneyard().len()) as u8,
        },
        Move::Pass => Action::Pass,
    };
    (s, a)
}

/// Every field of a state that the solver reads, and its hash.
fn same_state(a: &State, b: &State) -> bool {
    common::canon(a) == common::canon(b)
        && a.boneyard() == b.boneyard()
        && (a.board == 0 || (a.left, a.right) == (b.left, b.right))
        && a.hash() == b.hash()
}

/// (c): the sampled world rebuilds into a deal that replays the point.
fn consistent(point: &Point, w: &World, n_groups: usize) -> bool {
    point
        .deal_of(&w.groups[..n_groups], Some(w.state.boneyard()))
        .and_then(|d| point.replay(&d))
        .is_some_and(|s| same_state(&s, &w.state))
}

/// Every consistent assignment of the world tiles, by group sizes alone,
/// with its weight.
fn enumerate(point: &Point, w: &Worlds) -> Vec<(Vec<u32>, u64)> {
    let list: Vec<Tile> = w.tiles().iter().map(|&(t, _)| t).collect();
    let mut sizes: Vec<u8> = w.groups().iter().map(|&(_, n)| n).collect();
    let mut masks = vec![0u32; sizes.len()];
    let mut out = Vec::new();
    fn rec(
        i: usize,
        list: &[Tile],
        sizes: &mut [u8],
        masks: &mut [u32],
        f: &mut dyn FnMut(&[u32]),
    ) {
        if i == list.len() {
            f(masks);
            return;
        }
        for g in 0..sizes.len() {
            if sizes[g] > 0 {
                sizes[g] -= 1;
                masks[g] |= list[i].bit();
                rec(i + 1, list, sizes, masks, f);
                masks[g] &= !list[i].bit();
                sizes[g] += 1;
            }
        }
    }
    rec(0, &list, &mut sizes, &mut masks, &mut |m| {
        if let Some(d) = point.deal_of(m, None)
            && point.replay(&d).is_some()
        {
            out.push((m.to_vec(), point.weight(&d)));
        }
    });
    out
}

/// ln Gamma(x) for x > 0 (Lanczos, g = 7, nine coefficients).
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, &c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The upper regularized incomplete gamma function Q(a, x): a series below
/// x = a + 1, a continued fraction (Lentz) above.
fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let front = (-x + a * x.ln() - ln_gamma(a)).exp();
    if x < a + 1.0 {
        let (mut ap, mut del, mut sum) = (a, 1.0 / a, 1.0 / a);
        for _ in 0..1_000_000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        1.0 - sum * front
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1_000_000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-15 {
                break;
            }
        }
        front * h
    }
}

/// P(chi-square with df degrees of freedom >= x).
fn chi_square_p(x: f64, df: u64) -> f64 {
    gamma_q(df as f64 / 2.0, x / 2.0)
}

#[test]
fn chi_square_p_values() {
    // 95% quantiles of the chi-square distribution.
    for (x, df) in [
        (3.841_459, 1),
        (18.307_038, 10),
        (124.342_113, 100),
        (10_233.749_9, 10_000),
    ] {
        let p = chi_square_p(x, df);
        assert!((p - 0.05).abs() < 1e-4, "df {df}: p {p}");
    }
    assert!((chi_square_p(6.634_897, 1) - 0.01).abs() < 1e-5);
}

#[test]
fn sampler_counts_equal_the_world_counts() {
    let mut checked = 0u64;
    for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
        decisions(mode, 10, |_, _, _, info| {
            let w = Worlds::new(info).unwrap();
            let s = Sampler::new(info).unwrap();
            assert_eq!(s.count(), u128::from(w.count()));
            checked += 1;
        });
    }
    println!("sampler counts: {checked} decisions, equal to the M2-03 counts");
}

/// One set of T3 (a): its decisions' p-values and pooled statistic.
#[derive(Default)]
struct Set {
    decisions: u64,
    low_p: u64,
    chi: f64,
    df: u64,
    samples: u64,
    inconsistent: u64,
}

fn chi_square_set(set: &mut Set, c: u64, point: &Point, info: &InfoSet) {
    let w = Worlds::new(info).unwrap();
    let sampler = Sampler::new(info).unwrap();
    let worlds = enumerate(point, &w);
    let total: u64 = worlds.iter().map(|(_, wt)| wt).sum();
    let p_min = worlds
        .iter()
        .map(|&(_, wt)| wt as f64 / total as f64)
        .fold(1.0, f64::min);
    let n = (10.0 / p_min).ceil() as u64;
    let index: BTreeMap<Vec<u32>, usize> = worlds
        .iter()
        .enumerate()
        .map(|(i, (m, _))| (m.clone(), i))
        .collect();
    let mut observed = vec![0u64; worlds.len()];
    let mut rng = Rng::new(7_000_000 + 1_000 * c + set.decisions);
    let k = w.groups().len();
    for _ in 0..n {
        let world = sampler.sample(&mut rng);
        if !consistent(point, &world, k) {
            set.inconsistent += 1;
        }
        match index.get(&world.groups[..k]) {
            Some(&i) => observed[i] += 1,
            None => set.inconsistent += 1,
        }
    }
    let mut x = 0.0;
    for (i, &(_, wt)) in worlds.iter().enumerate() {
        let e = n as f64 * wt as f64 / total as f64;
        let d = observed[i] as f64 - e;
        x += d * d / e;
    }
    let df = worlds.len() as u64 - 1;
    set.low_p += u64::from(chi_square_p(x, df) < 0.01);
    set.chi += x;
    set.df += df;
    set.samples += n;
    set.decisions += 1;
}

fn qualifies(info: &InfoSet) -> bool {
    let w = Worlds::new(info).unwrap();
    (2..=2_000).contains(&w.count()) && multinomial(&w) <= 100_000
}

#[test]
fn sampled_worlds_match_exact_enumeration() {
    let (games, per_set, no_double_max) = if full() { (200, 100, 30) } else { (4, 3, 2) };
    let mut sets: [Set; 3] = Default::default();
    for (c, mode) in [(0u64, Mode::TwoVsTwo), (1, Mode::OneVsOne)] {
        let set = &mut sets[c as usize];
        'games: for g in 0..games {
            let record = common::random_record(mode, 100, 700_000 + g, true);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for ply in 0..round.moves.len() {
                    let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                    if qualifies(&info) {
                        let point = Point::new(&record, r, ply, info.seat);
                        chi_square_set(set, c, &point, &info);
                        if set.decisions == per_set {
                            break 'games;
                        }
                        break;
                    }
                    state.play(round.moves[ply]);
                }
            }
        }
    }
    // No-double 1v1 openings: every qualifying round-1 decision.
    let set = &mut sets[2];
    let mut g = 0u64;
    while set.decisions < no_double_max && g < 2_000 {
        let mut rng = Rng::new(710_000 + g);
        g += 1;
        let deal = no_double_deal(&mut rng);
        let (mut state, _) = State::first_round(Mode::OneVsOne, 100, &deal);
        let mut record = GameRecord {
            mode: Mode::OneVsOne,
            target: 100,
            rounds: vec![RoundRecord {
                deal,
                moves: vec![],
            }],
        };
        let mut moves = MoveList::new();
        while state.phase == Phase::Playing && set.decisions < no_double_max {
            let ply = record.rounds[0].moves.len();
            let info = InfoSet::new(&record, 0, ply, state.turn).unwrap();
            if qualifies(&info) {
                let point = Point::new(&record, 0, ply, info.seat);
                chi_square_set(set, 2, &point, &info);
            }
            state.legal_moves(&mut moves);
            let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
            state.play(mv);
            record.rounds[0].moves.push(mv);
        }
    }
    let names = ["2v2", "1v1", "1v1 no-double"];
    let mut parts = Vec::new();
    for (i, set) in sets.iter().enumerate() {
        let pooled = chi_square_p(set.chi, set.df);
        parts.push(format!(
            "[{}: decisions {} samples {} p<0.01 in {} pooled X2 {:.1} df {} p {:.4} inconsistent {}]",
            names[i], set.decisions, set.samples, set.low_p, set.chi, set.df, pooled, set.inconsistent
        ));
        assert_eq!(set.inconsistent, 0, "{}", names[i]);
        if full() {
            let (want, max_low) = if i == 2 { (30, 2) } else { (100, 5) };
            assert_eq!(set.decisions, want, "{}: too few decisions", names[i]);
            assert!(
                set.low_p <= max_low,
                "{}: {} low p-values",
                names[i],
                set.low_p
            );
            assert!(pooled >= 0.001, "{}: pooled p {pooled}", names[i]);
        }
    }
    println!(
        "sampler uniformity ({}): {}",
        if full() {
            "T3 (a) as registered"
        } else {
            "smoke"
        },
        parts.join(" ")
    );
}

#[test]
fn the_true_world_appears_at_the_uniform_rate() {
    let games = if full() { 200 } else { 3 };
    let mut parts = Vec::new();
    for (c, mode) in [(0u64, Mode::TwoVsTwo), (1, Mode::OneVsOne)] {
        let (mut hits, mut expected, mut variance) = (0u64, 0.0f64, 0.0f64);
        let (mut d, mut samples, mut inconsistent) = (0u64, 0u64, 0u64);
        for g in 0..games {
            let record = common::random_record(mode, 100, 700_000 + g, true);
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for ply in 0..round.moves.len() {
                    let info = InfoSet::new(&record, r, ply, state.turn).unwrap();
                    state.play(round.moves[ply]);
                    let other_no_double = matches!(info.opening, Some((s, t)) if s != info.seat && !t.is_double())
                        && mode == Mode::OneVsOne;
                    let sampler = Sampler::new(&info).unwrap();
                    let count = sampler.count();
                    if count <= 1 || other_no_double {
                        continue;
                    }
                    let w = Worlds::new(&info).unwrap();
                    let k = w.groups().len();
                    let mut truth = vec![0u32; k];
                    let world_tiles = w.tiles().iter().fold(0u32, |m, &(t, _)| m | t.bit());
                    for (t, g) in true_groups(&record, r, ply, info.seat) {
                        if world_tiles & t.bit() != 0 {
                            truth[g] |= t.bit();
                        }
                    }
                    let point = Point::new(&record, r, ply, info.seat);
                    let mut rng = Rng::new(7_500_000 + 1_000_000 * c + d);
                    for _ in 0..100 {
                        let world = sampler.sample(&mut rng);
                        hits += u64::from(world.groups[..k] == truth[..]);
                        if !consistent(&point, &world, k) {
                            inconsistent += 1;
                        }
                    }
                    let p = 1.0 / count as f64;
                    expected += 100.0 * p;
                    variance += 100.0 * p * (1.0 - p);
                    samples += 100;
                    d += 1;
                }
            }
        }
        let z = (hits as f64 - expected) / variance.sqrt();
        parts.push(format!(
            "[{mode:?}: decisions {d} samples {samples} true-world draws {hits} expected {expected:.1} z {z:.2} inconsistent {inconsistent}]"
        ));
        assert_eq!(inconsistent, 0, "{mode:?}");
        if full() {
            assert!(expected >= 200.0, "{mode:?}: expected {expected}");
            assert!(z.abs() <= 3.29, "{mode:?}: z {z}");
        }
    }
    println!(
        "sampler truth ({}): {}",
        if full() {
            "T3 (b) as registered"
        } else {
            "smoke"
        },
        parts.join(" ")
    );
}

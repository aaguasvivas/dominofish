//! The world sampler (M4-01; docs/PLAN.md, D17): draws worlds consistent
//! with an InfoSet, each in proportion to the number of consistent deals it
//! stands for, and builds each world's full state. It reads only the
//! InfoSet, never a true hand.
//!
//! A dynamic program over the tiles of `Worlds` (M2-03) counts, for each
//! tile and each vector of remaining group sizes, the ways to place the
//! later tiles. A world places each tile in turn in an allowed group with
//! probability proportional to the ways left after that choice, so every
//! assignment is drawn with probability 1 / count and no draw dead-ends.
//!
//! In 1v1 the boneyard order is a uniform shuffle of the boneyard group.
//! After a round-1 opening of the other seat with no double, an assignment
//! whose dealt batch holds m tiles of the opening tile's pip sum stands for
//! 1/m of the deal orders (the first of them in hand order opened), so it
//! is kept with probability 1/m and otherwise drawn again.

use std::collections::{BTreeMap, BTreeSet};

use crate::infoset::{InfoSet, NO_END};
use crate::rng::Rng;
use crate::rules::{Mode, NO_SEAT, Parts, Phase, State};
use crate::tile::{ALL, SUIT, Tile, tiles};
use crate::worlds::{Holder, Worlds};

/// Why no world can be drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleError {
    /// The other 1v1 seat drew and has not moved since (`Worlds::new`); a
    /// seat's own decisions never fall here.
    PendingDraw,
    /// No world is consistent with the InfoSet. A referee's InfoSet always
    /// has the true world, so this counts as a fallback (D17).
    NoWorld,
}

/// One drawn world.
#[derive(Clone, Copy, Debug)]
pub struct World {
    /// The tiles of each group, in `Worlds::groups` order.
    pub groups: [u32; 16],
    /// The full state at the InfoSet's point.
    pub state: State,
}

pub struct Sampler {
    /// The tiles in drawing order (most constrained first), with the
    /// groups each may be in.
    order: Vec<(Tile, u32)>,
    /// ways[i]: remaining group sizes (four bits per group) to the number
    /// of ways to place tiles i and later; only entries above 0.
    ways: Vec<BTreeMap<u64, u128>>,
    start: u64,
    holders: Vec<Holder>,
    /// The InfoSet's public fields, with the seat's own hand.
    base: Parts,
    /// The tiles the seat cannot see.
    hidden: u32,
    /// 1v1, after a round-1 opening of the other seat with no double: the
    /// opening tile's pip sum.
    opening_sum: Option<u32>,
}

/// A uniform integer in 0..n, exactly: Lemire's method for n below 2^32,
/// rejection over 128-bit draws above.
fn below(rng: &mut Rng, n: u128) -> u128 {
    if let Ok(small) = u32::try_from(n) {
        return u128::from(rng.below(small));
    }
    // Draws above u128::MAX - r would favor the low residues.
    let r = (u128::MAX % n + 1) % n;
    loop {
        let x = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
        if x <= u128::MAX - r {
            return x % n;
        }
    }
}

fn size(caps: u64, g: usize) -> u64 {
    (caps >> (4 * g)) & 0xF
}

impl Sampler {
    pub fn new(info: &InfoSet) -> Result<Sampler, SampleError> {
        let worlds = Worlds::new(info).ok_or(SampleError::PendingDraw)?;
        let groups = worlds.groups();
        let need: usize = groups.iter().map(|&(_, n)| usize::from(n)).sum();
        if need != worlds.tiles().len() {
            return Err(SampleError::NoWorld);
        }
        let mut order = worlds.tiles().to_vec();
        order.sort_by_key(|&(_, a)| a.count_ones());
        let start = groups
            .iter()
            .enumerate()
            .fold(0u64, |m, (g, &(_, n))| m | u64::from(n) << (4 * g));
        // The group sizes reachable after each tile, forward.
        let mut reach: Vec<BTreeSet<u64>> = vec![BTreeSet::from([start])];
        for &(_, allowed) in &order {
            let mut next = BTreeSet::new();
            for &caps in reach.last().expect("one layer per tile and the start") {
                for g in 0..groups.len() {
                    if allowed & (1 << g) != 0 && size(caps, g) != 0 {
                        next.insert(caps - (1 << (4 * g)));
                    }
                }
            }
            reach.push(next);
        }
        // The ways to finish from each, backward.
        let n = order.len();
        let mut ways: Vec<BTreeMap<u64, u128>> = vec![BTreeMap::new(); n + 1];
        if reach[n].contains(&0) {
            ways[n].insert(0, 1);
        }
        for i in (0..n).rev() {
            let allowed = order[i].1;
            let mut layer = BTreeMap::new();
            for &caps in &reach[i] {
                let mut w = 0u128;
                for g in 0..groups.len() {
                    if allowed & (1 << g) != 0 && size(caps, g) != 0 {
                        w += ways[i + 1]
                            .get(&(caps - (1 << (4 * g))))
                            .copied()
                            .unwrap_or(0);
                    }
                }
                if w > 0 {
                    layer.insert(caps, w);
                }
            }
            ways[i] = layer;
        }
        if !ways[0].contains_key(&start) {
            return Err(SampleError::NoWorld);
        }
        let opening_sum = match (info.mode, info.opening) {
            (Mode::OneVsOne, Some((opener, tile))) if opener != info.seat && !tile.is_double() => {
                Some(tile.pips())
            }
            _ => None,
        };
        let end = |e: u8| if e == NO_END { 0 } else { e };
        let mut hands = [0u32; 4];
        hands[info.seat as usize] = info.hand;
        Ok(Sampler {
            order,
            ways,
            start,
            holders: groups.iter().map(|&(h, _)| h).collect(),
            base: Parts {
                mode: info.mode,
                target: info.target,
                scores: info.scores,
                hands,
                board: info.board,
                left: end(info.left),
                right: end(info.right),
                turn: info.turn,
                starter: info.starter,
                last_played_by: info.last_played_by,
                passes: info.passes,
                phase: info.phase,
                round_winner: NO_SEAT,
            },
            hidden: info.hidden(),
            opening_sum,
        })
    }

    /// The number of consistent assignments of the world tiles (the count
    /// of M2-03).
    pub fn count(&self) -> u128 {
        self.ways[0][&self.start]
    }

    /// Draws one world. Allocates nothing.
    pub fn sample(&self, rng: &mut Rng) -> World {
        let groups = loop {
            let mut caps = self.start;
            let mut groups = [0u32; 16];
            for (i, &(t, allowed)) in self.order.iter().enumerate() {
                let mut x = below(rng, self.ways[i][&caps]);
                for (g, slot) in groups.iter_mut().enumerate().take(self.holders.len()) {
                    if allowed & (1 << g) == 0 || size(caps, g) == 0 {
                        continue;
                    }
                    let next = caps - (1 << (4 * g));
                    let w = self.ways[i + 1].get(&next).copied().unwrap_or(0);
                    if x < w {
                        caps = next;
                        *slot |= t.bit();
                        break;
                    }
                    x -= w;
                }
            }
            let kept = match self.opening_sum {
                None => true,
                Some(s) => {
                    let m = tiles(groups[0]).filter(|t| t.pips() == s).count();
                    rng.below(m as u32) == 0
                }
            };
            if kept {
                break groups;
            }
        };
        let mut parts = self.base;
        let mut pile = [Tile::from_index(0); 14];
        let mut pile_len = 0;
        for (g, holder) in self.holders.iter().enumerate() {
            match *holder {
                Holder::Hand { seat } => parts.hands[seat as usize] = groups[g],
                Holder::Batch { seat, .. } => parts.hands[seat as usize] |= groups[g] & self.hidden,
                Holder::Boneyard => {
                    for t in tiles(groups[g]) {
                        pile[pile_len] = t;
                        pile_len += 1;
                    }
                }
            }
        }
        for i in (1..pile_len).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            pile.swap(i, j);
        }
        World {
            groups,
            state: State::from_parts(&parts, &pile[..pile_len]),
        }
    }

    /// Whether `state` agrees with what the seat knows: its own hand, the
    /// hand sizes, the boneyard count, every tile in one place, and no hand
    /// holding a tile of its seat's void pips. The engine checks every world
    /// with it (D17); the replay check of test T3 is stronger.
    pub fn agrees(info: &InfoSet, state: &State) -> bool {
        let seats = info.mode.seats() as usize;
        let pile = state.boneyard().iter().fold(0u32, |m, t| m | t.bit());
        let mut seen = state.board | pile;
        let mut count = state.board.count_ones() + pile.count_ones();
        for x in 0..seats {
            let h = state.hands[x];
            let void = (0..7)
                .filter(|p| info.voids[x] & (1 << p) != 0)
                .fold(0u32, |m, p| m | SUIT[p]);
            if h.count_ones() != u32::from(info.hand_sizes[x]) || h & void != 0 || seen & h != 0 {
                return false;
            }
            seen |= h;
            count += h.count_ones();
        }
        seen == ALL
            && count == 28
            && state.hands[info.seat as usize] == info.hand
            && state.board == info.board
            && state.boneyard().len() == usize::from(info.boneyard_count)
            && state.phase == Phase::Playing
            && state.turn == info.turn
    }
}

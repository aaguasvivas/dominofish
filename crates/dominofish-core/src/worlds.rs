//! Consistent worlds (M2-03; docs/PLAN.md, D11): the assignments of the
//! tiles a seat cannot place to the holders they may be in, given its
//! InfoSet, and their exact count.
//!
//! 2v2: the holders are the three other hands, with their sizes; a tile may
//! be in a hand unless it shows one of that seat's void pips (hands never
//! grow, so a pass binds the seat's whole round).
//!
//! 1v1: the holders are the other seat's acquisition batches (its dealt hand,
//! then the non-fitting tiles of each of its draws) and the boneyard. The
//! tiles are the hidden ones and the other seat's played tiles, since which
//! batch a played tile came from is hidden too; the tile played right after a
//! draw is that draw's fitting tile and is pinned. A tile may be in a batch
//! when it avoids the ends of every draw and pass of that seat while the
//! batch held it (until the tile was played), and, for the dealt batch, the
//! round-1 exclusions. Each assignment stands for the same number of full
//! deal orders, so worlds are counted and later sampled per assignment.

use std::collections::HashMap;

use crate::infoset::{Action, InfoSet, NO_END};
use crate::rules::Mode;
use crate::tile::{SUIT, Tile, tiles};

/// Who holds a group of tiles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Holder {
    /// A 2v2 hand.
    Hand {
        seat: u8,
    },
    /// A 1v1 acquisition batch of `seat`: 0 is the dealt hand, k the
    /// non-fitting tiles of its k-th draw.
    Batch {
        seat: u8,
        index: u8,
    },
    Boneyard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worlds {
    groups: Vec<(Holder, u8)>,
    /// Each tile to place, with a bit per group it may be in.
    tiles: Vec<(Tile, u32)>,
}

fn suits(pips: u8) -> u32 {
    (0..7)
        .filter(|p| pips & (1 << p) != 0)
        .fold(0, |m, p| m | SUIT[p as usize])
}

fn end_bits(left: u8, right: u8) -> u8 {
    if left == NO_END {
        0
    } else {
        (1 << left) | (1 << right)
    }
}

impl Worlds {
    /// Groups with their sizes, and tiles with the groups they may be in.
    pub fn from_groups(groups: &[(Holder, u8)], tiles: &[(Tile, u32)]) -> Worlds {
        assert!(groups.len() <= 16, "at most 16 groups");
        Worlds {
            groups: groups.to_vec(),
            tiles: tiles.to_vec(),
        }
    }

    /// The worlds consistent with `info`. None while the other 1v1 seat's
    /// draw is unresolved (it drew and has not moved since), where the hand
    /// holds a drawn tile that may fit; a seat's own decisions never fall
    /// there.
    pub fn new(info: &InfoSet) -> Option<Worlds> {
        match info.mode {
            Mode::TwoVsTwo => Some(Self::two_vs_two(info)),
            Mode::OneVsOne => Self::one_vs_one(info),
        }
    }

    fn two_vs_two(info: &InfoSet) -> Worlds {
        let seats: Vec<u8> = (0..4).filter(|&s| s != info.seat).collect();
        let groups: Vec<(Holder, u8)> = seats
            .iter()
            .map(|&seat| (Holder::Hand { seat }, info.hand_sizes[seat as usize]))
            .collect();
        let tiles = tiles(info.hidden())
            .map(|t| {
                let allowed = seats
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| t.bit() & suits(info.voids[**s as usize]) == 0)
                    .fold(0u32, |m, (g, _)| m | 1 << g);
                (t, allowed)
            })
            .collect();
        Worlds { groups, tiles }
    }

    fn one_vs_one(info: &InfoSet) -> Option<Worlds> {
        let other = 1 - info.seat;
        let history = info.history();
        // Batches: the dealt hand, then one per draw of `other`, each with
        // the ply it starts at (the dealt hand at -1) and its size.
        let mut batches: Vec<(i32, u8)> = vec![(-1, 7)];
        // The other seat's no-fit events: (ply, excluded tiles).
        let mut events: Vec<(i32, u32)> = Vec::new();
        // The other seat's played tiles with their ply, and the pinned ones.
        let mut played: Vec<(Tile, i32)> = Vec::new();
        for (k, pm) in history.iter().enumerate() {
            if pm.seat != other {
                continue;
            }
            let k = k as i32;
            match pm.action {
                Action::Draw { count } => {
                    // Its own fitting tile is played at the seat's next move,
                    // which is the next history entry; with none yet, the draw
                    // is unresolved.
                    let next = history.get(k as usize + 1)?;
                    let fit = matches!(next.action, Action::Play { .. });
                    batches.push((k, count - u8::from(fit)));
                    events.push((k, suits(end_bits(pm.left, pm.right))));
                }
                Action::Pass => events.push((k, suits(end_bits(pm.left, pm.right)))),
                Action::Play { tile, .. } => {
                    let after_draw = k > 0
                        && matches!(history[k as usize - 1].action, Action::Draw { .. })
                        && history[k as usize - 1].seat == other;
                    if !after_draw {
                        played.push((tile, k));
                    }
                }
            }
        }
        // Round 1's automatic tile of the other seat came from its dealt hand.
        let opening = info.opening.filter(|(s, _)| *s == other).map(|(_, t)| t);
        let n_batches = batches.len();
        let boneyard = n_batches;
        let mut groups: Vec<(Holder, u8)> = batches
            .iter()
            .enumerate()
            .map(|(i, &(_, size))| {
                (
                    Holder::Batch {
                        seat: other,
                        index: i as u8,
                    },
                    size,
                )
            })
            .collect();
        groups.push((Holder::Boneyard, info.boneyard_count));
        if groups.len() > 16 {
            return None;
        }
        // The tiles excluded from batch b for a tile held until ply `until`.
        let excluded = |b: usize, until: i32| {
            let start = batches[b].0;
            let mut m = if b == 0 {
                info.dealt_excluded[other as usize]
            } else {
                0
            };
            for &(ply, ex) in &events {
                if ply >= start && ply < until {
                    m |= ex;
                }
            }
            m
        };
        let allowed = |t: Tile, until: i32, hidden: bool| {
            let mut a = 0u32;
            for (b, &(start, _)) in batches.iter().enumerate() {
                if start < until && t.bit() & excluded(b, until) == 0 {
                    a |= 1 << b;
                }
            }
            if hidden {
                a |= 1 << boneyard;
            }
            a
        };
        let now = history.len() as i32;
        let mut list: Vec<(Tile, u32)> = Vec::new();
        if let Some(t) = opening {
            list.push((t, 1));
        }
        for &(t, ply) in &played {
            list.push((t, allowed(t, ply, false)));
        }
        for t in tiles(info.hidden()) {
            list.push((t, allowed(t, now, true)));
        }
        Some(Worlds {
            groups,
            tiles: list,
        })
    }

    pub fn groups(&self) -> &[(Holder, u8)] {
        &self.groups
    }

    pub fn tiles(&self) -> &[(Tile, u32)] {
        &self.tiles
    }

    /// The number of consistent worlds.
    pub fn count(&self) -> u64 {
        self.count_where(None)
    }

    /// The number of consistent worlds with `tile` in group `group`.
    pub fn count_with(&self, tile: Tile, group: usize) -> u64 {
        self.count_where(Some((tile, group)))
    }

    fn count_where(&self, fixed: Option<(Tile, usize)>) -> u64 {
        let need: u32 = self.groups.iter().map(|&(_, n)| u32::from(n)).sum();
        if need as usize != self.tiles.len() {
            return 0;
        }
        // Remaining group sizes, four bits per group.
        let start = self
            .groups
            .iter()
            .enumerate()
            .fold(0u64, |m, (g, &(_, n))| m | u64::from(n) << (4 * g));
        let mut layer: HashMap<u64, u64> = HashMap::from([(start, 1)]);
        // Most constrained tiles first keeps the layers small.
        let mut order: Vec<(Tile, u32)> = self.tiles.clone();
        order.sort_by_key(|&(_, a)| a.count_ones());
        for (t, mut allowed) in order {
            if let Some((ft, g)) = fixed
                && ft == t
            {
                allowed &= 1 << g;
            }
            let mut next: HashMap<u64, u64> = HashMap::with_capacity(layer.len() * 2);
            for (&caps, &ways) in &layer {
                for g in 0..self.groups.len() {
                    if allowed & (1 << g) != 0 && (caps >> (4 * g)) & 0xF != 0 {
                        *next.entry(caps - (1 << (4 * g))).or_insert(0) += ways;
                    }
                }
            }
            layer = next;
        }
        layer.get(&0).copied().unwrap_or(0)
    }
}

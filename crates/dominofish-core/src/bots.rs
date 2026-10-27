//! Baseline bots (M2-04). Each one decides from the InfoSet of its own seat
//! only (hard constraint 1), breaking ties with the RNG it is given, so a bot
//! with a fixed RNG stream is deterministic. Definitions (docs/PLAN.md, D12):
//!
//! - random: uniform over the canonical legal moves.
//! - heaviest (bota gorda): the play with the most pips.
//! - accompanied (la mas acompanada): the play whose new end shows the pip
//!   of which the bot keeps the most tiles in hand, then the heaviest.
//! - rule: counts tiles and uses pass voids. It scores each play by its
//!   ends after the play: ends the next seat is void in (it must pass or
//!   draw), ends the partner is void in (avoided, 2v2), its own tiles that
//!   can follow, and the unseen tiles that could; then the heaviest.
//!
//! A draw or a pass is never a choice (it is the only legal move).

use crate::infoset::InfoSet;
use crate::rng::Rng;
use crate::rules::{End, Mode, Move, MoveList};
use crate::tile::{SUIT, Tile};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bot {
    Random,
    Heaviest,
    Accompanied,
    Rule,
}

impl Bot {
    pub const ALL: [Bot; 4] = [Bot::Random, Bot::Heaviest, Bot::Accompanied, Bot::Rule];

    pub fn name(self) -> &'static str {
        match self {
            Bot::Random => "random",
            Bot::Heaviest => "heaviest",
            Bot::Accompanied => "accompanied",
            Bot::Rule => "rule",
        }
    }

    pub fn from_name(name: &str) -> Option<Bot> {
        Bot::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The bot's move for its seat, which must be on turn: uniform over
    /// `policy`.
    pub fn decide(self, info: &InfoSet, rng: &mut Rng) -> Move {
        let mut moves = MoveList::new();
        self.policy(info, &mut moves);
        let first = moves.as_slice()[0];
        // A draw or a pass takes no random number, as before policy existed,
        // so every recorded bot game replays the same.
        if !matches!(first, Move::Play { .. }) {
            return first;
        }
        moves.as_slice()[rng.below(moves.len() as u32) as usize]
    }

    /// The moves the bot chooses among, each with the same probability: its
    /// exact policy (the AIVAT action terms of M5 need it). A draw or a pass
    /// is the only legal move when it is legal.
    pub fn policy(self, info: &InfoSet, out: &mut MoveList) {
        info.legal_moves(out);
        assert!(!out.is_empty(), "the bot's seat must be on turn");
        if !matches!(out.as_slice()[0], Move::Play { .. }) || self == Bot::Random {
            return;
        }
        let legal = *out;
        match self {
            Bot::Random => {}
            Bot::Heaviest => best(info, &legal, out, |_, tile, _| i64::from(tile.pips())),
            Bot::Accompanied => best(info, &legal, out, accompanied_score),
            Bot::Rule => best(info, &legal, out, rule_score),
        }
    }
}

/// The plays with the highest score into `tied`, in generator order. The
/// score sees the InfoSet, the tile, and the end it goes to.
fn best(
    info: &InfoSet,
    moves: &MoveList,
    tied: &mut MoveList,
    score: impl Fn(&InfoSet, Tile, End) -> i64,
) {
    let mut top = i64::MIN;
    tied.clear();
    for &mv in moves.as_slice() {
        let Move::Play { tile, end } = mv else {
            continue;
        };
        let s = score(info, tile, end);
        if s > top {
            top = s;
            tied.clear();
        }
        if s == top {
            tied.push(mv);
        }
    }
}

/// Tiles of `mask` that show `left` or `right`.
fn follow(mask: u32, (left, right): (u8, u8)) -> u32 {
    (mask & (SUIT[left as usize] | SUIT[right as usize])).count_ones()
}

/// The pip a play exposes: the pip at the end it went to (a double keeps
/// its own pip there); on an empty board the higher pip.
fn exposed(info: &InfoSet, tile: Tile, end: End) -> u8 {
    let (left, right) = info.ends_after(tile, end);
    match (info.board == 0, end) {
        (true, _) => left.max(right),
        (false, End::Left) => left,
        (false, End::Right) => right,
    }
}

fn accompanied_score(info: &InfoSet, tile: Tile, end: End) -> i64 {
    let rest = info.hand & !tile.bit();
    let kept = (rest & SUIT[exposed(info, tile, end) as usize]).count_ones();
    i64::from(kept) * 100 + i64::from(tile.pips())
}

fn rule_score(info: &InfoSet, tile: Tile, end: End) -> i64 {
    let ends = info.ends_after(tile, end);
    let (left, right) = ends;
    let pips = (1u8 << left) | (1 << right);
    let next = info.mode.next(info.seat) as usize;
    let mut score = 0i64;
    // The next seat surely cannot follow: it passes (2v2) or draws (1v1).
    if info.voids[next] & pips == pips {
        score += 400;
    } else if info.voids[next] & pips != 0 {
        score += 100;
    }
    if info.mode == Mode::TwoVsTwo {
        let partner = info.mode.next(info.mode.next(info.seat)) as usize;
        if info.voids[partner] & pips == pips {
            score -= 300;
        }
    }
    let rest = info.hand & !tile.bit();
    let unseen = info.hidden();
    score += 30 * i64::from(follow(rest, ends));
    score -= 10 * i64::from(follow(unseen, ends));
    score * 100 + i64::from(tile.pips())
}

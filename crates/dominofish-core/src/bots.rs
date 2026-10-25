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
use crate::rules::{Mode, Move, MoveList};
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

    /// The bot's move for its seat, which must be on turn.
    pub fn decide(self, info: &InfoSet, rng: &mut Rng) -> Move {
        let mut moves = MoveList::new();
        info.legal_moves(&mut moves);
        assert!(!moves.is_empty(), "decide needs the seat on turn");
        let first = moves.as_slice()[0];
        if !matches!(first, Move::Play { .. }) {
            return first;
        }
        match self {
            Bot::Random => moves.as_slice()[rng.below(moves.len() as u32) as usize],
            Bot::Heaviest => best(info, &moves, rng, |_, tile, _| i64::from(tile.pips())),
            Bot::Accompanied => best(info, &moves, rng, accompanied_score),
            Bot::Rule => best(info, &moves, rng, rule_score),
        }
    }
}

/// The play with the highest score, ties uniform by the RNG. The score sees
/// the InfoSet, the tile, and the ends after the play.
fn best(
    info: &InfoSet,
    moves: &MoveList,
    rng: &mut Rng,
    score: impl Fn(&InfoSet, Tile, (u8, u8)) -> i64,
) -> Move {
    let mut top = i64::MIN;
    let mut tied = MoveList::new();
    for &mv in moves.as_slice() {
        let Move::Play { tile, end } = mv else {
            continue;
        };
        let s = score(info, tile, info.ends_after(tile, end));
        if s > top {
            top = s;
            tied.clear();
        }
        if s == top {
            tied.push(mv);
        }
    }
    tied.as_slice()[rng.below(tied.len() as u32) as usize]
}

/// Tiles of `mask` that show `left` or `right`.
fn follow(mask: u32, (left, right): (u8, u8)) -> u32 {
    (mask & (SUIT[left as usize] | SUIT[right as usize])).count_ones()
}

/// The pip a play exposes: the end it changed; on an empty board the higher
/// one; a double on its own pip changes nothing and exposes that pip.
fn exposed(info: &InfoSet, ends: (u8, u8)) -> u8 {
    if info.board == 0 {
        ends.0.max(ends.1)
    } else if ends.0 != info.left {
        ends.0
    } else {
        ends.1
    }
}

fn accompanied_score(info: &InfoSet, tile: Tile, ends: (u8, u8)) -> i64 {
    let rest = info.hand & !tile.bit();
    let kept = (rest & SUIT[exposed(info, ends) as usize]).count_ones();
    i64::from(kept) * 100 + i64::from(tile.pips())
}

fn rule_score(info: &InfoSet, tile: Tile, ends: (u8, u8)) -> i64 {
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

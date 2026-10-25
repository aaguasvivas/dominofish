//! Information sets (hard constraint 1): what one seat knows at one point of
//! a round. Built from a game record, it keeps only the seat's own hand and
//! public facts; no field holds a tile hidden from that seat. Deductions
//! follow docs/RULES.md and digest 1.7.

use crate::record::{GameRecord, RecordError};
use crate::rules::{End, Mode, Move, NO_SEAT, Phase};
use crate::tile::{ALL, Tile, tiles};

/// Enough entries for any round (digest 1.8: at most 97 moves in 2v2, 67 in
/// 1v1).
pub const MAX_HISTORY: usize = 128;

/// An end value for "the board is empty".
pub const NO_END: u8 = 7;

/// What everyone sees of a move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// A play; on an empty board `end` is the orientation (docs/RULES.md).
    Play {
        tile: Tile,
        end: End,
    },
    /// A 1v1 draw of `count` tiles, whose identities only the drawer sees.
    Draw {
        count: u8,
    },
    Pass,
}

/// One move of the round as everyone sees it, with the ends before it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PublicMove {
    pub seat: u8,
    pub action: Action,
    pub left: u8,
    pub right: u8,
}

const NO_MOVE: PublicMove = PublicMove {
    seat: NO_SEAT,
    action: Action::Pass,
    left: NO_END,
    right: NO_END,
};

/// What `seat` knows at one point of a round.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InfoSet {
    pub mode: Mode,
    pub target: u16,
    pub scores: [u16; 2],
    /// The seat whose knowledge this is.
    pub seat: u8,
    /// That seat's own hand.
    pub hand: u32,
    pub board: u32,
    /// Open ends, NO_END on an empty board.
    pub left: u8,
    pub right: u8,
    pub turn: u8,
    /// The round's opener (la mano).
    pub starter: u8,
    pub last_played_by: u8,
    pub passes: u8,
    pub phase: Phase,
    /// Tiles in each seat's hand (0 for the unused seats in 1v1).
    pub hand_sizes: [u8; 4],
    pub boneyard_count: u8,
    /// Round 1's opener and automatic tile.
    pub opening: Option<(u8, Tile)>,
    history: [PublicMove; MAX_HISTORY],
    history_len: u8,
    /// Pips each seat surely lacks in its current hand, as bit masks (bit p
    /// for pip p). 2v2: the ends at each of its passes, for the whole round,
    /// since hands never grow. 1v1: the ends at its latest draw, from its
    /// next move on, and at its passes after it; older draws bind only the
    /// tiles held then, which the history records.
    pub voids: [u8; 4],
    /// 1v1 round 1: tiles each seat's dealt hand cannot have held, from the
    /// opener rule (docs/RULES.md R4); empty otherwise.
    pub dealt_excluded: [u32; 4],
}

fn pip_bits(left: u8, right: u8) -> u8 {
    if left == NO_END {
        0
    } else {
        (1 << left) | (1 << right)
    }
}

impl InfoSet {
    /// What `seat` knows after the first `ply` moves of round `round`.
    pub fn new(
        record: &GameRecord,
        round: usize,
        ply: usize,
        seat: u8,
    ) -> Result<InfoSet, RecordError> {
        if seat >= record.mode.seats() {
            return Err(RecordError::NoSuchPosition);
        }
        let (mut state, opening_tile) = record.round_start(round)?;
        let moves = &record.rounds[round].moves;
        if ply > moves.len() {
            return Err(RecordError::NoSuchPosition);
        }
        let opening = opening_tile.map(|t| (state.starter, t));
        let mut history = [NO_MOVE; MAX_HISTORY];
        let mut voids = [0u8; 4];
        let mut pending = [0u8; 4];
        for (k, &mv) in moves[..ply].iter().enumerate() {
            if !state.is_legal(mv) {
                return Err(RecordError::IllegalMove { round, ply: k });
            }
            let (s, empty) = (state.turn, state.board == 0);
            let (left, right) = if empty {
                (NO_END, NO_END)
            } else {
                (state.left, state.right)
            };
            let pile = state.boneyard().len();
            state.play(mv);
            let action = match mv {
                Move::Play { tile, end } => Action::Play { tile, end },
                Move::Draw => Action::Draw {
                    count: (pile - state.boneyard().len()) as u8,
                },
                Move::Pass => Action::Pass,
            };
            // A 1v1 draw's void holds once its last tile, which may fit, is
            // gone: after the drawer's next move (the forced play, or a pass
            // when the boneyard ran out with no fit). Until then the hand
            // holds a tile of unknown identity that may show an end, and the
            // other new tiles may show older ends, so nothing is certain.
            let p = s as usize;
            match (record.mode, action) {
                (Mode::TwoVsTwo, Action::Pass) => voids[p] |= pip_bits(left, right),
                (Mode::OneVsOne, Action::Draw { .. }) => {
                    voids[p] = 0;
                    pending[p] = pip_bits(left, right);
                }
                (Mode::OneVsOne, _) => {
                    voids[p] |= core::mem::take(&mut pending[p]);
                    if action == Action::Pass {
                        voids[p] |= pip_bits(left, right);
                    }
                }
                _ => {}
            }
            history[k] = PublicMove {
                seat: s,
                action,
                left,
                right,
            };
        }
        let mut hand_sizes = [0u8; 4];
        for (size, hand) in hand_sizes.iter_mut().zip(state.hands) {
            *size = hand.count_ones() as u8;
        }
        let dealt_excluded = match (record.mode, opening) {
            (Mode::OneVsOne, Some((opener, tile))) => opening_exclusions(opener, tile),
            _ => [0; 4],
        };
        Ok(InfoSet {
            mode: state.mode,
            target: state.target,
            scores: state.scores,
            seat,
            hand: state.hands[seat as usize],
            board: state.board,
            left: if state.board == 0 { NO_END } else { state.left },
            right: if state.board == 0 {
                NO_END
            } else {
                state.right
            },
            turn: state.turn,
            starter: state.starter,
            last_played_by: state.last_played_by,
            passes: state.passes,
            phase: state.phase,
            hand_sizes,
            boneyard_count: state.boneyard().len() as u8,
            opening,
            history,
            history_len: ply as u8,
            voids,
            dealt_excluded,
        })
    }

    /// The round's moves so far, as everyone saw them.
    pub fn history(&self) -> &[PublicMove] {
        &self.history[..self.history_len as usize]
    }

    /// The tiles this seat cannot see: in other hands or the boneyard.
    pub fn hidden(&self) -> u32 {
        ALL & !self.board & !self.hand
    }
}

/// 1v1 round 1 (docs/RULES.md R4; digest 1.7). A double d opened: neither
/// dealt hand held a higher double. No double opened: neither hand held a
/// double; the opener's other tiles have at most its tile's pip sum S, and
/// the other seat's tiles have at most S when it comes later in seat order
/// and less than S when it comes earlier (ties go to the earlier seat).
fn opening_exclusions(opener: u8, tile: Tile) -> [u32; 4] {
    let doubles = (0..=6u8).fold(0u32, |m, d| m | Tile::new(d, d).bit());
    let mut out = [0u32; 4];
    if tile.is_double() {
        let higher = (tile.low() + 1..=6).fold(0u32, |m, d| m | Tile::new(d, d).bit());
        out[0] = higher;
        out[1] = higher;
    } else {
        let s = tile.pips();
        let above = |strict: bool| {
            tiles(ALL)
                .filter(|t| if strict { t.pips() > s } else { t.pips() >= s })
                .fold(0u32, |m, t| m | t.bit())
        };
        let other = 1 - opener;
        out[opener as usize] = doubles | above(true);
        out[other as usize] = doubles | above(other > opener);
    }
    out
}

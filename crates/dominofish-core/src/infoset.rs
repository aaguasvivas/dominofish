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

impl InfoSet {
    /// The seat's canonical legal moves when it is on turn, from its own hand
    /// and the public facts alone (the same set as State::legal_moves).
    pub fn legal_moves(&self, out: &mut crate::rules::MoveList) {
        out.clear();
        if self.phase != Phase::Playing || self.turn != self.seat {
            return;
        }
        if self.board == 0 {
            for tile in tiles(self.hand) {
                out.push(Move::Play {
                    tile,
                    end: End::Left,
                });
            }
            return;
        }
        let fit_left = self.hand & crate::tile::SUIT[self.left as usize];
        let fit_right = if self.left == self.right {
            0
        } else {
            self.hand & crate::tile::SUIT[self.right as usize]
        };
        for tile in tiles(fit_left | fit_right) {
            if fit_left & tile.bit() != 0 {
                out.push(Move::Play {
                    tile,
                    end: End::Left,
                });
            }
            if fit_right & tile.bit() != 0 {
                out.push(Move::Play {
                    tile,
                    end: End::Right,
                });
            }
        }
        if out.is_empty() {
            out.push(if self.boneyard_count > 0 {
                Move::Draw
            } else {
                Move::Pass
            });
        }
    }

    /// The ends after `mv`, a play of this seat.
    pub fn ends_after(&self, tile: Tile, end: End) -> (u8, u8) {
        match (self.board == 0, end) {
            (true, End::Left) => (tile.low(), tile.high()),
            (true, End::Right) => (tile.high(), tile.low()),
            (false, End::Left) => (tile.other(self.left), self.right),
            (false, End::Right) => (self.left, tile.other(self.right)),
        }
    }
}

impl InfoSet {
    /// The text form (docs/FORMATS.md, InfoSet text): one line of key=value
    /// fields, the same bytes for equal InfoSets.
    pub fn to_text(&self) -> String {
        use core::fmt::Write as _;
        let mode = self.mode;
        let seat = |s: u8| if s == NO_SEAT { "-" } else { mode.seat_name(s) };
        let tiles_text = |m: u32| {
            tiles(m)
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let ends = |l: u8, r: u8| {
            if l == NO_END {
                "--".to_string()
            } else {
                format!("{l}{r}")
            }
        };
        let mut out = format!(
            "v=1 mode={} target={} scores={},{} seat={} hand={} board={:x} ends={} turn={} starter={} last={} passes={} phase={} sizes={},{},{},{} pile={} opening={}",
            mode.name(),
            self.target,
            self.scores[0],
            self.scores[1],
            seat(self.seat),
            tiles_text(self.hand),
            self.board,
            ends(self.left, self.right),
            seat(self.turn),
            seat(self.starter),
            seat(self.last_played_by),
            self.passes,
            match self.phase {
                Phase::Playing => "P",
                Phase::RoundOver => "R",
                Phase::Finished => "F",
            },
            self.hand_sizes[0],
            self.hand_sizes[1],
            self.hand_sizes[2],
            self.hand_sizes[3],
            self.boneyard_count,
            self.opening
                .map_or("-".into(), |(s, t)| format!("{}:{t}", seat(s))),
        );
        let _ = write!(
            out,
            " voids={:x},{:x},{:x},{:x} excluded={:x},{:x},{:x},{:x} history=",
            self.voids[0],
            self.voids[1],
            self.voids[2],
            self.voids[3],
            self.dealt_excluded[0],
            self.dealt_excluded[1],
            self.dealt_excluded[2],
            self.dealt_excluded[3]
        );
        let entries: Vec<String> = self
            .history()
            .iter()
            .map(|pm| {
                let action = match pm.action {
                    Action::Play { tile, end } => {
                        format!("p{tile}{}", if end == End::Left { "L" } else { "R" })
                    }
                    Action::Draw { count } => format!("d{count}"),
                    Action::Pass => "x".into(),
                };
                format!("{}:{action}:{}", seat(pm.seat), ends(pm.left, pm.right))
            })
            .collect();
        out.push_str(if entries.is_empty() { "-" } else { "" });
        out.push_str(&entries.join(";"));
        out
    }

    /// Parses `to_text`; None on any malformed field or on facts that no
    /// game reaches (see `consistent`).
    pub fn from_text(text: &str) -> Option<InfoSet> {
        Self::parse(text).filter(InfoSet::consistent)
    }

    /// The checks that keep every consumer from indexing out of range: a
    /// real seat on turn and to view, ends that the board shows, at most 7
    /// tiles in hand on an empty board, a hand of the stated size, and
    /// scores below the 512 the hash covers.
    fn consistent(&self) -> bool {
        let seats = self.mode.seats();
        let real = |s: u8| s < seats;
        let shown = |pip: u8| tiles(self.board).any(|t| t.has(pip));
        let ends_ok = if self.board == 0 {
            self.left == NO_END && self.right == NO_END && self.hand.count_ones() <= 7
        } else {
            self.left < NO_END && self.right < NO_END && shown(self.left) && shown(self.right)
        };
        real(self.seat)
            && real(self.turn)
            && real(self.starter)
            && (self.last_played_by == NO_SEAT || real(self.last_played_by))
            && ends_ok
            && self.hand & self.board == 0
            && u32::from(self.hand_sizes[self.seat as usize]) == self.hand.count_ones()
            && self.hand_sizes.iter().all(|&n| n <= 21)
            && self.passes <= 3
            && self.boneyard_count <= 14
            && self.target >= 1
            && self.target <= crate::rules::MAX_TARGET
            && self.scores.iter().all(|&s| s < 512)
    }

    fn parse(text: &str) -> Option<InfoSet> {
        use crate::record::{parse_seat, parse_tile};
        let field = |key: &str| {
            text.split(' ')
                .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
        };
        if field("v")? != "1" {
            return None;
        }
        let mode = match field("mode")? {
            "1v1" => Mode::OneVsOne,
            "2v2" => Mode::TwoVsTwo,
            _ => return None,
        };
        let seat = |s: &str| {
            if s == "-" {
                Some(NO_SEAT)
            } else {
                parse_seat(mode, s)
            }
        };
        let pair = |s: &str| -> Option<[u16; 2]> {
            let (a, b) = s.split_once(',')?;
            Some([a.parse().ok()?, b.parse().ok()?])
        };
        let four = |s: &str, radix: u32| -> Option<[u32; 4]> {
            let v: Vec<u32> = s
                .split(',')
                .map(|x| u32::from_str_radix(x, radix).ok())
                .collect::<Option<_>>()?;
            v.try_into().ok()
        };
        let ends = |s: &str| -> Option<(u8, u8)> {
            if s == "--" {
                return Some((NO_END, NO_END));
            }
            let b = s.as_bytes();
            (b.len() == 2 && b.iter().all(|c| (b'0'..=b'6').contains(c)))
                .then(|| (b[0] - b'0', b[1] - b'0'))
        };
        let hand = match field("hand")? {
            "" => 0,
            h => h
                .split(',')
                .map(parse_tile)
                .collect::<Option<Vec<_>>>()?
                .iter()
                .fold(0u32, |m, t| m | t.bit()),
        };
        let (left, right) = ends(field("ends")?)?;
        let sizes = four(field("sizes")?, 10)?;
        let voids = four(field("voids")?, 16)?;
        let opening = match field("opening")? {
            "-" => None,
            o => {
                let (s, t) = o.split_once(':')?;
                Some((seat(s)?, parse_tile(t)?))
            }
        };
        let mut history = [NO_MOVE; MAX_HISTORY];
        let mut len = 0;
        let h = field("history")?;
        if h != "-" {
            for (k, entry) in h.split(';').enumerate() {
                let mut parts = entry.split(':');
                let (s, a, e) = (parts.next()?, parts.next()?, parts.next()?);
                let action = match a.as_bytes().first()? {
                    b'p' => {
                        let body = a.get(1..)?;
                        let (t, end) = if let Some(t) = body.strip_suffix('L') {
                            (t, End::Left)
                        } else {
                            (body.strip_suffix('R')?, End::Right)
                        };
                        Action::Play {
                            tile: parse_tile(t)?,
                            end,
                        }
                    }
                    b'd' => Action::Draw {
                        count: a[1..].parse().ok()?,
                    },
                    b'x' => Action::Pass,
                    _ => return None,
                };
                let (l, r) = ends(e)?;
                *history.get_mut(k)? = PublicMove {
                    seat: seat(s)?,
                    action,
                    left: l,
                    right: r,
                };
                len = k + 1;
            }
        }
        Some(InfoSet {
            mode,
            target: field("target")?.parse().ok()?,
            scores: pair(field("scores")?)?,
            seat: seat(field("seat")?)?,
            hand,
            board: u32::from_str_radix(field("board")?, 16).ok()?,
            left,
            right,
            turn: seat(field("turn")?)?,
            starter: seat(field("starter")?)?,
            last_played_by: seat(field("last")?)?,
            passes: field("passes")?.parse().ok()?,
            phase: match field("phase")? {
                "P" => Phase::Playing,
                "R" => Phase::RoundOver,
                "F" => Phase::Finished,
                _ => return None,
            },
            hand_sizes: sizes.map(|x| x as u8),
            boneyard_count: field("pile")?.parse().ok()?,
            opening,
            history,
            history_len: len as u8,
            voids: voids.map(|x| x as u8),
            dealt_excluded: four(field("excluded")?, 16)?,
        })
    }
}

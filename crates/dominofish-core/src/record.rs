//! Game records: each round's deal in Capi's layout and its moves in order
//! (docs/FORMATS.md gives the text form from M2-02). A record holds every
//! hidden tile; only the referee and record tools may read it directly.

use crate::rules::{Deal, Mode, Move, Phase, State};
use crate::tile::Tile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundRecord {
    pub deal: Deal,
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    pub mode: Mode,
    pub target: u16,
    pub rounds: Vec<RoundRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The record has no such round or ply.
    NoSuchPosition,
    /// A move the referee rejects.
    IllegalMove { round: usize, ply: usize },
    /// A round's moves end before the round does, or run past its end.
    RoundNotOver { round: usize },
}

impl GameRecord {
    /// The state at the start of `round`, after replaying every earlier
    /// round in full, with round 1's automatic tile.
    pub fn round_start(&self, round: usize) -> Result<(State, Option<Tile>), RecordError> {
        let first = self.rounds.first().ok_or(RecordError::NoSuchPosition)?;
        let (mut state, tile) = State::first_round(self.mode, self.target, &first.deal);
        if round == 0 {
            return Ok((state, Some(tile)));
        }
        for (r, rec) in self.rounds.iter().enumerate() {
            if r == round {
                return Ok((state, None));
            }
            for (ply, &mv) in rec.moves.iter().enumerate() {
                if !state.is_legal(mv) {
                    return Err(RecordError::IllegalMove { round: r, ply });
                }
                state.play(mv);
            }
            if state.phase != Phase::RoundOver {
                return Err(RecordError::RoundNotOver { round: r });
            }
            let next = self.rounds.get(r + 1).ok_or(RecordError::NoSuchPosition)?;
            state = state.next_round(&next.deal);
        }
        Err(RecordError::NoSuchPosition)
    }
}

/// A text-format error with its 1-based line number (0: the whole record).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub reason: String,
}

fn err(line: usize, reason: impl Into<String>) -> ParseError {
    ParseError {
        line,
        reason: reason.into(),
    }
}

/// A tile written "a-b" with pips 0 to 6, in either order.
pub fn parse_tile(s: &str) -> Option<Tile> {
    let (a, b) = s.split_once('-')?;
    let (a, b): (u8, u8) = (a.parse().ok()?, b.parse().ok()?);
    (a <= 6 && b <= 6).then(|| Tile::new(a, b))
}

/// The seat index of a seat name in `mode` (n, e, s, w in 2v2; n, s in 1v1).
pub fn parse_seat(mode: Mode, s: &str) -> Option<u8> {
    (0..mode.seats()).find(|&x| mode.seat_name(x) == s)
}

/// A move as "play <a-b> <left|right>", "draw", or "pass" (docs/FORMATS.md).
pub fn move_text(mv: Move) -> String {
    match mv {
        Move::Play { tile, end } => format!(
            "play {tile} {}",
            if end == crate::rules::End::Left {
                "left"
            } else {
                "right"
            }
        ),
        Move::Draw => "draw".into(),
        Move::Pass => "pass".into(),
    }
}

/// Parses the words of `move_text`.
pub fn parse_move(words: &[&str]) -> Option<Move> {
    use crate::rules::End;
    match words {
        ["play", tile, end] => Some(Move::Play {
            tile: parse_tile(tile)?,
            end: match *end {
                "left" => End::Left,
                "right" => End::Right,
                _ => return None,
            },
        }),
        ["draw"] => Some(Move::Draw),
        ["pass"] => Some(Move::Pass),
        _ => None,
    }
}

impl GameRecord {
    /// The text form (docs/FORMATS.md, Game records).
    pub fn to_text(&self) -> String {
        use core::fmt::Write as _;
        let mode = match self.mode {
            Mode::OneVsOne => "1v1",
            Mode::TwoVsTwo => "2v2",
        };
        let mut out = format!("dominofish-record 1\nmode {mode}\ntarget {}\n", self.target);
        for (r, round) in self.rounds.iter().enumerate() {
            out.push_str("deal");
            for t in round.deal.tiles() {
                let _ = write!(out, " {t}");
            }
            out.push('\n');
            let (mut state, _) = self.round_start(r).expect("a valid record");
            for &mv in &round.moves {
                let _ = writeln!(out, "{} {}", self.mode.seat_name(state.turn), move_text(mv));
                state.play(mv);
            }
        }
        out.push_str("end\n");
        out
    }

    /// Parses the text form and checks it by replay: every move is legal and
    /// made by the seat on turn, and every round but the last is over.
    pub fn from_text(text: &str) -> Result<GameRecord, ParseError> {
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
        let mut header = |want: &str| -> Result<(usize, String), ParseError> {
            let (n, l) = lines
                .next()
                .ok_or_else(|| err(0, format!("missing {want} line")))?;
            let rest = l
                .strip_prefix(want)
                .and_then(|r| r.strip_prefix(' '))
                .ok_or_else(|| err(n, format!("expected {want}")))?;
            Ok((n, rest.to_string()))
        };
        let (n, version) = header("dominofish-record")?;
        if version != "1" {
            return Err(err(n, format!("unknown record version {version}")));
        }
        let (n, mode) = header("mode")?;
        let mode = match mode.as_str() {
            "1v1" => Mode::OneVsOne,
            "2v2" => Mode::TwoVsTwo,
            _ => return Err(err(n, format!("mode must be 1v1 or 2v2, got {mode}"))),
        };
        let (n, target) = header("target")?;
        let target: u16 = target
            .parse()
            .ok()
            .filter(|t| (1..=crate::rules::MAX_TARGET).contains(t))
            .ok_or_else(|| err(n, format!("bad target {target}")))?;
        let mut rounds: Vec<RoundRecord> = Vec::new();
        let mut seats: Vec<Vec<(usize, u8)>> = Vec::new();
        let mut ended = false;
        for (n, line) in lines {
            if ended {
                return Err(err(n, "text after end"));
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["end"] => ended = true,
                ["deal", tiles @ ..] => {
                    if tiles.len() != 28 {
                        return Err(err(n, format!("a deal has 28 tiles, got {}", tiles.len())));
                    }
                    let mut t = [Tile::from_index(0); 28];
                    for (k, s) in tiles.iter().enumerate() {
                        t[k] = parse_tile(s).ok_or_else(|| err(n, format!("bad tile {s}")))?;
                    }
                    let deal = Deal::from_tiles(t)
                        .ok_or_else(|| err(n, "a deal must hold every tile once"))?;
                    rounds.push(RoundRecord {
                        deal,
                        moves: vec![],
                    });
                    seats.push(vec![]);
                }
                [seat, mv @ ..] => {
                    let seat =
                        parse_seat(mode, seat).ok_or_else(|| err(n, format!("bad seat {seat}")))?;
                    let mv = parse_move(mv).ok_or_else(|| err(n, format!("bad move: {line}")))?;
                    let round = rounds
                        .last_mut()
                        .ok_or_else(|| err(n, "a move before the first deal"))?;
                    round.moves.push(mv);
                    seats.last_mut().expect("one per round").push((n, seat));
                }
                [] => {}
            }
        }
        if !ended {
            return Err(err(0, "missing end line"));
        }
        if rounds.is_empty() {
            return Err(err(0, "no deal"));
        }
        let record = GameRecord {
            mode,
            target,
            rounds,
        };
        for (r, round) in record.rounds.iter().enumerate() {
            let (mut state, _) = record
                .round_start(r)
                .map_err(|e| err(0, format!("{e:?}")))?;
            for (&mv, &(n, seat)) in round.moves.iter().zip(&seats[r]) {
                if state.turn != seat {
                    return Err(err(n, format!("not {}'s turn", mode.seat_name(seat))));
                }
                if !state.is_legal(mv) {
                    return Err(err(n, "illegal move"));
                }
                state.play(mv);
            }
            if r + 1 < record.rounds.len() && state.phase != Phase::RoundOver {
                return Err(err(
                    0,
                    format!("round {} is not over before the next deal", r + 1),
                ));
            }
            if state.phase == Phase::Finished && r + 1 < record.rounds.len() {
                return Err(err(0, "a deal after the game ended"));
            }
        }
        Ok(record)
    }
}

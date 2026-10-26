//! The engine line protocol (docs/FORMATS.md, Engine protocol): the referee
//! and each engine exchange one text line per message over the engine's
//! stdin and stdout. An engine receives only its own hand, its own drawn
//! tiles, and public moves. Parsing and printing only; the processes and
//! pipes belong to the command line.

use crate::infoset::{Action, InfoSet};
use crate::record::{GameRecord, move_text, parse_move, parse_seat, parse_tile};
use crate::rules::{Mode, Move, Phase};
use crate::tile::Tile;

pub const VERSION: u32 = 1;

/// Referee to engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToEngine {
    /// `dominofish <version>`.
    Hello { version: u32 },
    /// `header <text>`: the match header, which the engine echoes back.
    Header(String),
    /// `game mode=<1v1|2v2> target=<t> seat=<seat> seed=<u64>`.
    Game {
        mode: Mode,
        target: u16,
        seat: u8,
        seed: u64,
    },
    /// `round <r> scores=<s0>,<s1> leader=<seat> hand=<a-b>,...`: the
    /// engine's dealt hand; round 1's leader is the opener, whose automatic
    /// tile follows as `opening`.
    Round {
        round: u32,
        scores: [u16; 2],
        leader: u8,
        hand: Vec<Tile>,
    },
    /// `opening <seat> <a-b>`.
    Opening { seat: u8, tile: Tile },
    /// `drew <a-b>,...`: the tiles this engine drew, in order, sent to the
    /// drawer only, before the public `played <seat> draw <count>`.
    Drew(Vec<Tile>),
    /// `played <seat> play <a-b> <left|right>`, `played <seat> draw <count>`,
    /// or `played <seat> pass`.
    Played { seat: u8, action: Action },
    /// `infoset <text>`: the InfoSet the referee builds for the engine's
    /// seat (docs/FORMATS.md, InfoSet text), sent before each `go`.
    InfoSet(Box<InfoSet>),
    /// `go`: the engine's turn.
    Go,
    /// `roundover scores=<s0>,<s1>`.
    RoundOver { scores: [u16; 2] },
    /// `gameover scores=<s0>,<s1>`.
    GameOver { scores: [u16; 2] },
    /// `quit`.
    Quit,
}

/// Engine to referee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FromEngine {
    /// `dominofish <version> ready budgets=<b> threads=<n> tables=<t>`: the
    /// engine's own settings, which the referee puts in the match header
    /// (- for none).
    Ready {
        version: u32,
        budgets: String,
        threads: u32,
        tables: String,
    },
    /// `header <text>`, echoing the referee's header.
    Header(String),
    /// `move play <a-b> <left|right>`, `move draw`, or `move pass`.
    Move(Move),
    /// `info <text>`: free text the referee logs and ignores.
    Info(String),
}

fn mode_text(mode: Mode) -> &'static str {
    match mode {
        Mode::OneVsOne => "1v1",
        Mode::TwoVsTwo => "2v2",
    }
}

fn tiles_text(tiles: &[Tile]) -> String {
    tiles
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_tiles(s: &str) -> Option<Vec<Tile>> {
    s.split(',').map(parse_tile).collect()
}

fn parse_scores(s: &str) -> Option<[u16; 2]> {
    let (a, b) = s.split_once(',')?;
    Some([a.parse().ok()?, b.parse().ok()?])
}

fn field<'a>(words: &[&'a str], key: &str) -> Option<&'a str> {
    words
        .iter()
        .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
}

impl ToEngine {
    /// The line, with seats named for `mode`.
    pub fn to_line(&self, mode: Mode) -> String {
        let seat = |s: u8| mode.seat_name(s);
        match self {
            ToEngine::Hello { version } => format!("dominofish {version}"),
            ToEngine::Header(text) => format!("header {text}"),
            ToEngine::Game {
                mode,
                target,
                seat,
                seed,
            } => format!(
                "game mode={} target={target} seat={} seed={seed}",
                mode_text(*mode),
                mode.seat_name(*seat)
            ),
            ToEngine::Round {
                round,
                scores,
                leader,
                hand,
            } => format!(
                "round {round} scores={},{} leader={} hand={}",
                scores[0],
                scores[1],
                seat(*leader),
                tiles_text(hand)
            ),
            ToEngine::Opening { seat: s, tile } => format!("opening {} {tile}", seat(*s)),
            ToEngine::Drew(tiles) => format!("drew {}", tiles_text(tiles)),
            ToEngine::Played { seat: s, action } => {
                let what = match *action {
                    Action::Play { tile, end } => move_text(Move::Play { tile, end }),
                    Action::Draw { count } => format!("draw {count}"),
                    Action::Pass => "pass".into(),
                };
                format!("played {} {what}", seat(*s))
            }
            ToEngine::InfoSet(info) => format!("infoset {}", info.to_text()),
            ToEngine::Go => "go".into(),
            ToEngine::RoundOver { scores } => {
                format!("roundover scores={},{}", scores[0], scores[1])
            }
            ToEngine::GameOver { scores } => format!("gameover scores={},{}", scores[0], scores[1]),
            ToEngine::Quit => "quit".into(),
        }
    }

    /// Parses a line; `mode` names the seats (a `game` line carries its own).
    pub fn parse(line: &str, mode: Mode) -> Option<ToEngine> {
        let words: Vec<&str> = line.split_whitespace().collect();
        Some(match words.as_slice() {
            ["dominofish", v] => ToEngine::Hello {
                version: v.parse().ok()?,
            },
            ["header", ..] => ToEngine::Header(line.strip_prefix("header ")?.to_string()),
            ["game", ..] => {
                let mode = match field(&words, "mode")? {
                    "1v1" => Mode::OneVsOne,
                    "2v2" => Mode::TwoVsTwo,
                    _ => return None,
                };
                ToEngine::Game {
                    mode,
                    target: field(&words, "target")?.parse().ok()?,
                    seat: parse_seat(mode, field(&words, "seat")?)?,
                    seed: field(&words, "seed")?.parse().ok()?,
                }
            }
            ["round", r, ..] => ToEngine::Round {
                round: r.parse().ok()?,
                scores: parse_scores(field(&words, "scores")?)?,
                leader: parse_seat(mode, field(&words, "leader")?)?,
                hand: parse_tiles(field(&words, "hand")?)?,
            },
            ["opening", s, t] => ToEngine::Opening {
                seat: parse_seat(mode, s)?,
                tile: parse_tile(t)?,
            },
            ["drew", t] => ToEngine::Drew(parse_tiles(t)?),
            ["played", s, rest @ ..] => ToEngine::Played {
                seat: parse_seat(mode, s)?,
                action: match rest {
                    ["draw", n] => Action::Draw {
                        count: n.parse().ok()?,
                    },
                    ["pass"] => Action::Pass,
                    _ => match parse_move(rest)? {
                        Move::Play { tile, end } => Action::Play { tile, end },
                        _ => return None,
                    },
                },
            },
            ["infoset", ..] => ToEngine::InfoSet(Box::new(InfoSet::from_text(
                line.strip_prefix("infoset ")?,
            )?)),
            ["go"] => ToEngine::Go,
            ["roundover", s] => ToEngine::RoundOver {
                scores: parse_scores(s.strip_prefix("scores=")?)?,
            },
            ["gameover", s] => ToEngine::GameOver {
                scores: parse_scores(s.strip_prefix("scores=")?)?,
            },
            ["quit"] => ToEngine::Quit,
            _ => return None,
        })
    }
}

impl FromEngine {
    pub fn to_line(&self) -> String {
        match self {
            FromEngine::Ready {
                version,
                budgets,
                threads,
                tables,
            } => format!(
                "dominofish {version} ready budgets={budgets} threads={threads} tables={tables}"
            ),
            FromEngine::Header(text) => format!("header {text}"),
            FromEngine::Move(mv) => format!("move {}", move_text(*mv)),
            FromEngine::Info(text) => format!("info {text}"),
        }
    }

    pub fn parse(line: &str) -> Option<FromEngine> {
        let words: Vec<&str> = line.split_whitespace().collect();
        Some(match words.as_slice() {
            ["dominofish", v, "ready", b, n, t] => FromEngine::Ready {
                version: v.parse().ok()?,
                budgets: b.strip_prefix("budgets=")?.to_string(),
                threads: n.strip_prefix("threads=")?.parse().ok()?,
                tables: t.strip_prefix("tables=")?.to_string(),
            },
            ["header", ..] => FromEngine::Header(line.strip_prefix("header ")?.to_string()),
            ["move", rest @ ..] => FromEngine::Move(parse_move(rest)?),
            ["info", ..] => FromEngine::Info(line.strip_prefix("info ")?.to_string()),
            _ => return None,
        })
    }
}

/// Every message `seat` receives over a recorded game, from `dominofish` to
/// `quit`, with `infoset` and `go` before each of its own moves (the engine
/// replies to each `go` with that move). The match harness sends the same
/// sequence while the game is played.
pub fn messages_for(record: &GameRecord, seat: u8, header: &str, seed: u64) -> Vec<ToEngine> {
    let mut out = vec![
        ToEngine::Hello { version: VERSION },
        ToEngine::Header(header.into()),
        ToEngine::Game {
            mode: record.mode,
            target: record.target,
            seat,
            seed,
        },
    ];
    for (r, round) in record.rounds.iter().enumerate() {
        let (mut state, opening) = record.round_start(r).expect("a valid record");
        out.push(ToEngine::Round {
            round: r as u32 + 1,
            scores: state.scores,
            leader: state.starter,
            hand: round.deal.hand(seat).to_vec(),
        });
        if let Some(tile) = opening {
            out.push(ToEngine::Opening {
                seat: state.starter,
                tile,
            });
        }
        for (ply, &mv) in round.moves.iter().enumerate() {
            let mover = state.turn;
            if mover == seat {
                let info = InfoSet::new(record, r, ply, seat).expect("a valid record");
                out.push(ToEngine::InfoSet(Box::new(info)));
                out.push(ToEngine::Go);
            }
            let pile = state.boneyard().to_vec();
            state.play(mv);
            let action = match mv {
                Move::Play { tile, end } => Action::Play { tile, end },
                Move::Draw => {
                    let drawn: Vec<Tile> = pile[state.boneyard().len()..]
                        .iter()
                        .rev()
                        .copied()
                        .collect();
                    if mover == seat {
                        out.push(ToEngine::Drew(drawn.clone()));
                    }
                    Action::Draw {
                        count: drawn.len() as u8,
                    }
                }
                Move::Pass => Action::Pass,
            };
            out.push(ToEngine::Played {
                seat: mover,
                action,
            });
        }
        match state.phase {
            Phase::RoundOver => out.push(ToEngine::RoundOver {
                scores: state.scores,
            }),
            Phase::Finished => out.push(ToEngine::GameOver {
                scores: state.scores,
            }),
            Phase::Playing => {}
        }
    }
    out.push(ToEngine::Quit);
    out
}

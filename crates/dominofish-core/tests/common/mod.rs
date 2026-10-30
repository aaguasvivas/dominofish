//! Replays Capi trace files (docs/FORMATS.md, Golden traces and Edge
//! scenarios) through the referee, comparing the opener, every event, and
//! every canonical state, and returns what Rust did for further checks.

#![allow(dead_code)]

pub mod oracle;

use std::fmt::Write as _;
use std::path::PathBuf;

use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::referee::Referee;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, NO_SEAT, Phase, State};
use dominofish_core::tile::Tile;

pub fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

pub fn seat_text(mode: Mode, seat: u8) -> &'static str {
    if seat == NO_SEAT {
        "-"
    } else {
        mode.seat_name(seat)
    }
}

/// The canonical state in the trace format.
pub fn canon(s: &State) -> String {
    let phase = match s.phase {
        Phase::Playing => "P",
        Phase::RoundOver => "R",
        Phase::Finished => "F",
    };
    let hands = match s.mode {
        Mode::TwoVsTwo => s.hands,
        Mode::OneVsOne => [s.hands[0], 0, s.hands[1], 0],
    };
    let mut out = format!(
        "{phase} {} {} {} {} {} {}",
        s.scores[0],
        s.scores[1],
        seat_text(s.mode, s.turn),
        seat_text(s.mode, s.starter),
        seat_text(s.mode, s.last_played_by),
        s.passes
    );
    for h in hands {
        let _ = write!(out, " {h:x}");
    }
    let _ = write!(out, " {:x}", s.board);
    if s.board == 0 {
        out.push_str(" - -");
    } else {
        let _ = write!(out, " {} {}", s.left, s.right);
    }
    let _ = write!(out, " {}", s.boneyard().len());
    out
}

pub fn event_text(mode: Mode, event: Option<Event>) -> String {
    let seat = |x| seat_text(mode, x);
    match event {
        None => "-".into(),
        Some(Event::Domino { seat: s, points }) => format!("domino {} {points}", seat(s)),
        Some(Event::Capicua { seat: s, points }) => format!("capicua {} {points}", seat(s)),
        Some(Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        }) => format!(
            "tranque {} {} {blocker_pips} {rival_pips} {} {points}",
            seat(blocker),
            seat(rival),
            seat(winner)
        ),
        Some(Event::PaseCorrido { team }) => format!("corrido {team}"),
        Some(Event::PaseSalida { team }) => format!("salida {team}"),
    }
}

fn parse_tile(s: &str) -> (u8, u8) {
    let (a, b) = s.split_once('-').expect("tile a-b");
    (a.parse().expect("pip"), b.parse().expect("pip"))
}

fn seat_index(mode: Mode, text: &str) -> u8 {
    (0..mode.seats())
        .find(|&s| mode.seat_name(s) == text)
        .expect("seat")
}

/// The exact-replay map of docs/RULES.md (Capi intents to moves).
fn parse_intent(state: &State, words: &[&str]) -> Move {
    match words {
        ["play", tile, end] => {
            let (a, b) = parse_tile(tile);
            let end = if state.board == 0 {
                if a <= b { End::Left } else { End::Right }
            } else if *end == "left" {
                End::Left
            } else {
                End::Right
            };
            Move::Play {
                tile: Tile::new(a, b),
                end,
            }
        }
        ["draw"] => Move::Draw,
        ["pass"] => Move::Pass,
        _ => panic!("bad intent {words:?}"),
    }
}

/// One move as Rust played it.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub before: State,
    pub mv: Move,
    pub event: Option<Event>,
    pub after: State,
}

/// One game (golden traces) or one round (edge scenarios) as Rust played it.
#[derive(Clone, Debug, Default)]
pub struct Played {
    pub name: String,
    pub init: Option<State>,
    pub opening: Option<(u8, Tile)>,
    pub steps: Vec<Step>,
    /// Moves Capi rejected at a state, which Rust must reject too.
    pub rejected: Vec<(State, Move)>,
    /// Lines of this game or round that differed from Capi's.
    pub mismatches: usize,
}

#[derive(Default)]
pub struct Report {
    pub games: Vec<Played>,
    pub moves: usize,
    pub mismatches: usize,
    pub errors: Vec<String>,
}

/// Replays one trace file.
pub fn replay(path: &PathBuf) -> Report {
    let text = std::fs::read_to_string(path).expect("read trace");
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    let mut report = Report::default();
    let mut referee: Option<Referee> = None;
    let mut mode = Mode::TwoVsTwo;
    let mut target = 100;
    let mut start: Option<(u8, [u16; 2])> = None;
    let mut seed = String::new();
    let mut current = Played::default();
    let mut mismatches_before = 0;
    let fail = |report: &mut Report, line: usize, seed: &str, what: String| {
        report
            .errors
            .push(format!("ERROR {file}:{line} seed {seed}: {what}"));
        report.mismatches += 1;
    };
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.starts_with('#') {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        let rest = rest.trim_start();
        match tag {
            "C" => current.name = rest.split(' ').next().unwrap_or("").to_string(),
            "G" => {
                mismatches_before = report.mismatches;
                let w: Vec<&str> = rest.split(' ').collect();
                mode = if w[0] == "1v1" {
                    Mode::OneVsOne
                } else {
                    Mode::TwoVsTwo
                };
                target = w[1].parse().expect("target");
                seed = w[2].to_string();
                referee = None;
                start = None;
            }
            "L" => {
                let w: Vec<&str> = rest.split(' ').collect();
                let scores = [w[1].parse().expect("score"), w[2].parse().expect("score")];
                start = Some((seat_index(mode, w[0]), scores));
            }
            "D" => {
                let mut tiles = [Tile::from_index(0); 28];
                for (k, v) in rest.split(',').enumerate() {
                    tiles[k] = Tile::from_index(v.parse().expect("tile index"));
                }
                let deal = Deal::from_tiles(tiles).expect("a full deal");
                match (referee.as_mut(), start) {
                    (None, None) => {
                        let (r, tile) = Referee::new(mode, target, &deal);
                        current.opening = Some((r.state().starter, tile));
                        current.init = Some(*r.state());
                        referee = Some(r);
                    }
                    (None, Some((leader, scores))) => {
                        let r =
                            Referee::from_state(State::dealt(mode, target, scores, &deal, leader));
                        current.init = Some(*r.state());
                        referee = Some(r);
                    }
                    (Some(r), _) => {
                        if r.next_round(&deal).is_err() {
                            fail(
                                &mut report,
                                n,
                                &seed,
                                "deal while the round is not over".into(),
                            );
                        }
                    }
                }
            }
            "O" => {
                let (seat, tile) = rest.split_once(' ').expect("O seat tile");
                let (a, b) = parse_tile(tile);
                let (got_seat, got_tile) = current.opening.expect("O after D");
                if seat_text(mode, got_seat) != seat || got_tile != Tile::new(a, b) {
                    let got = format!("{} {got_tile}", seat_text(mode, got_seat));
                    fail(
                        &mut report,
                        n,
                        &seed,
                        format!("opener want {seat} {tile} got {got}"),
                    );
                }
            }
            "S" => {
                let got = canon(referee.as_ref().expect("S after D").state());
                if got != rest {
                    fail(
                        &mut report,
                        n,
                        &seed,
                        format!("state want [{rest}] got [{got}]"),
                    );
                }
            }
            "X" => {
                let words: Vec<&str> = rest.split(" | ").next().unwrap().split(' ').collect();
                let state = *referee.as_ref().expect("X after D").state();
                let mv = parse_intent(&state, &words[1..]);
                if state.is_legal(mv) {
                    fail(
                        &mut report,
                        n,
                        &seed,
                        format!("Capi rejects {mv:?}, Rust accepts it"),
                    );
                }
                current.rejected.push((state, mv));
            }
            "M" => {
                report.moves += 1;
                let parts: Vec<&str> = rest.split(" | ").collect();
                let words: Vec<&str> = parts[0].split(' ').collect();
                let r = referee.as_mut().expect("M after D");
                let before = *r.state();
                if seat_text(mode, before.turn) != words[0] {
                    let got = seat_text(mode, before.turn);
                    fail(
                        &mut report,
                        n,
                        &seed,
                        format!("turn want {} got {got}", words[0]),
                    );
                    continue;
                }
                let mv = parse_intent(&before, &words[1..]);
                match r.play(mv) {
                    Err(e) => fail(
                        &mut report,
                        n,
                        &seed,
                        format!("referee rejected {mv:?}: {e:?}"),
                    ),
                    Ok(event) => {
                        let ev = event_text(mode, event);
                        if ev != parts[1] {
                            fail(
                                &mut report,
                                n,
                                &seed,
                                format!("event want [{}] got [{ev}]", parts[1]),
                            );
                        }
                        let got = canon(r.state());
                        if got != parts[2] {
                            fail(
                                &mut report,
                                n,
                                &seed,
                                format!("state want [{}] got [{got}]", parts[2]),
                            );
                        }
                        current.steps.push(Step {
                            before,
                            mv,
                            event,
                            after: *r.state(),
                        });
                    }
                }
            }
            "E" | "Z" => {
                let phase = referee.as_ref().map(|r| r.state().phase);
                let ok = if tag == "E" {
                    phase == Some(Phase::Finished)
                } else {
                    phase.is_some_and(|p| p != Phase::Playing)
                };
                if !ok {
                    fail(
                        &mut report,
                        n,
                        &seed,
                        format!("end line with phase {phase:?}"),
                    );
                }
                current.mismatches = report.mismatches - mismatches_before;
                report.games.push(std::mem::take(&mut current));
            }
            _ => fail(&mut report, n, &seed, format!("unknown line {line}")),
        }
    }
    report
}

/// A seeded game of random legal moves, recorded. With `raw`, a lead is
/// sometimes flipped and a play on equal ends sometimes goes right, so the
/// record holds non-canonical moves too.
pub fn random_record(mode: Mode, target: u16, seed: u64, raw: bool) -> GameRecord {
    let mut rng = Rng::new(seed);
    let mut rounds = vec![RoundRecord {
        deal: Deal::shuffled(&mut rng),
        moves: vec![],
    }];
    let (mut state, _) = State::first_round(mode, target, &rounds[0].deal);
    let mut moves = MoveList::new();
    loop {
        match state.phase {
            Phase::Finished => break,
            Phase::RoundOver => {
                let deal = Deal::shuffled(&mut rng);
                state = state.next_round(&deal);
                rounds.push(RoundRecord {
                    deal,
                    moves: vec![],
                });
            }
            Phase::Playing => {
                state.legal_moves(&mut moves);
                let mut mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                if let Move::Play {
                    tile,
                    end: End::Left,
                } = mv
                {
                    let mirror = Move::Play {
                        tile,
                        end: End::Right,
                    };
                    if raw && rng.below(2) == 0 && state.is_legal(mirror) {
                        mv = mirror;
                    }
                }
                state.play(mv);
                rounds.last_mut().unwrap().moves.push(mv);
            }
        }
    }
    GameRecord {
        mode,
        target,
        rounds,
    }
}

/// W for `target` from the rounds after the first of seeded bot games to
/// 200 (D20's recursion on bot rounds instead of PIMC's): a table whose
/// values vary with the scores, for tests of the win objective.
pub fn bot_equity(mode: Mode, target: u16) -> dominofish_core::equity::Equity {
    use dominofish_core::bots::Bot;
    use dominofish_core::equity::{Equity, rescore};
    let mut rounds = Vec::new();
    for (k, &bot) in Bot::ALL.iter().enumerate() {
        for g in 0..15u64 {
            let record =
                dominofish_core::referee::play_bots(mode, 200, [bot; 4], 8000 + 100 * k as u64 + g);
            for r in 1..record.rounds.len() {
                let (start, _) = record.round_start(r).unwrap();
                let round = &record.rounds[r];
                rounds.push(rescore(mode, &round.deal, start.starter, &round.moves));
            }
        }
    }
    Equity::build(target, &rounds)
}

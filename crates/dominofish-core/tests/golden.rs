//! M1-04: replays the golden traces in fixtures/golden (generated from Capi's
//! engine by tools/capi-oracle/traces.mjs) through the referee and compares
//! the opener, every event, and the canonical state after every move. Needs
//! no Capi checkout. Format: docs/FORMATS.md, Golden traces.

use std::fmt::Write as _;
use std::path::PathBuf;

use dominofish_core::referee::Referee;
use dominofish_core::rules::{Deal, End, Event, Mode, Move, NO_SEAT, Phase, State};
use dominofish_core::tile::Tile;

fn seat_text(mode: Mode, seat: u8) -> &'static str {
    if seat == NO_SEAT {
        "-"
    } else {
        mode.seat_name(seat)
    }
}

/// The canonical state in the trace format.
fn canon(s: &State) -> String {
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

fn event_text(mode: Mode, event: Option<Event>) -> String {
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

#[derive(Default)]
struct Tally {
    files: usize,
    games: usize,
    moves: usize,
    mismatches: usize,
}

fn replay(path: &PathBuf, tally: &mut Tally) {
    let text = std::fs::read_to_string(path).expect("read trace");
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let mut referee: Option<Referee> = None;
    let mut mode = Mode::TwoVsTwo;
    let mut target = 100;
    let mut seed = "";
    let mut opening = None;
    let mut fail = |line: usize, seed: &str, what: String| {
        if tally.mismatches < 10 {
            println!("ERROR golden {name}:{line} seed {seed}: {what}");
        }
        tally.mismatches += 1;
    };
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.starts_with('#') {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        let rest = rest.trim_start();
        match tag {
            "G" => {
                let w: Vec<&str> = rest.split(' ').collect();
                mode = if w[0] == "1v1" {
                    Mode::OneVsOne
                } else {
                    Mode::TwoVsTwo
                };
                target = w[1].parse().expect("target");
                seed = w[2];
                referee = None;
                tally.games += 1;
            }
            "D" => {
                let mut tiles = [Tile::from_index(0); 28];
                for (k, v) in rest.split(',').enumerate() {
                    tiles[k] = Tile::from_index(v.parse().expect("tile index"));
                }
                let deal = Deal::from_tiles(tiles).expect("a full deal");
                match referee.as_mut() {
                    None => {
                        let (r, tile) = Referee::new(mode, target, &deal);
                        opening = Some((r.state().starter, tile));
                        referee = Some(r);
                    }
                    Some(r) => {
                        if r.next_round(&deal).is_err() {
                            fail(n, seed, "deal while the round is not over".into());
                        }
                    }
                }
            }
            "O" => {
                let (seat, tile) = rest.split_once(' ').expect("O seat tile");
                let (a, b) = parse_tile(tile);
                let (got_seat, got_tile) = opening.expect("O after D");
                if seat_text(mode, got_seat) != seat || got_tile != Tile::new(a, b) {
                    fail(
                        n,
                        seed,
                        format!(
                            "opener want {seat} {tile} got {} {got_tile}",
                            seat_text(mode, got_seat)
                        ),
                    );
                }
            }
            "S" => {
                let got = canon(referee.as_ref().expect("S after D").state());
                if got != rest {
                    fail(n, seed, format!("state want [{rest}] got [{got}]"));
                }
            }
            "M" => {
                tally.moves += 1;
                let parts: Vec<&str> = rest.split(" | ").collect();
                let words: Vec<&str> = parts[0].split(' ').collect();
                let r = referee.as_mut().expect("M after D");
                let state = *r.state();
                if seat_text(mode, state.turn) != words[0] {
                    fail(
                        n,
                        seed,
                        format!("turn want {} got {}", words[0], seat_text(mode, state.turn)),
                    );
                    continue;
                }
                let mv = parse_intent(&state, &words[1..]);
                match r.play(mv) {
                    Err(e) => fail(n, seed, format!("referee rejected {mv:?}: {e:?}")),
                    Ok(event) => {
                        let ev = event_text(mode, event);
                        if ev != parts[1] {
                            fail(n, seed, format!("event want [{}] got [{ev}]", parts[1]));
                        }
                        let got = canon(r.state());
                        if got != parts[2] {
                            fail(n, seed, format!("state want [{}] got [{got}]", parts[2]));
                        }
                    }
                }
            }
            "E" => {
                let phase = referee.as_ref().map(|r| r.state().phase);
                if phase != Some(Phase::Finished) {
                    fail(n, seed, format!("game end with phase {phase:?}"));
                }
            }
            _ => fail(n, seed, format!("unknown line {line}")),
        }
    }
    tally.files += 1;
}

#[test]
fn golden_traces_replay_with_zero_mismatches() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("fixtures/golden exists")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt") && !p.ends_with("coverage.txt"))
        .collect();
    paths.sort();
    let mut tally = Tally::default();
    for p in &paths {
        replay(p, &mut tally);
    }
    println!(
        "golden: files {} games {} moves {} mismatches {}",
        tally.files, tally.games, tally.moves, tally.mismatches
    );
    assert!(tally.files >= 6 && tally.games > 0);
    assert_eq!(tally.mismatches, 0);
}

//! The dominofish command line.

use std::fmt::Write as _;
use std::process::ExitCode;

use lexopt::ValueExt;

use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, Phase, State};

const USAGE: &str = "usage: dominofish selfplay --mode 1v1|2v2 --seed N [--target 100|200]";

fn main() -> ExitCode {
    match run() {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ERROR {e}");
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<String, String> {
    use lexopt::prelude::*;
    let mut parser = lexopt::Parser::from_env();
    let command = match parser.next().map_err(|e| e.to_string())? {
        Some(Value(v)) => v.string().map_err(|e| e.to_string())?,
        _ => return Err("missing command".into()),
    };
    match command.as_str() {
        "selfplay" => {
            let (mut mode, mut seed, mut target) = (None, None, 100u16);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("target") => target = parse_target(&value(&mut parser)?)?,
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            let mode = mode.ok_or("--mode is required")?;
            let seed = seed.ok_or("--seed is required")?;
            Ok(selfplay(mode, seed, target))
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn value(parser: &mut lexopt::Parser) -> Result<String, String> {
    parser
        .value()
        .map_err(|e| e.to_string())?
        .string()
        .map_err(|e| e.to_string())
}

fn parse_mode(s: &str) -> Result<Mode, String> {
    match s {
        "1v1" => Ok(Mode::OneVsOne),
        "2v2" => Ok(Mode::TwoVsTwo),
        _ => Err(format!("--mode must be 1v1 or 2v2, got {s}")),
    }
}

fn parse_num(s: &str) -> Result<u64, String> {
    s.parse().map_err(|_| format!("not a number: {s}"))
}

fn parse_target(s: &str) -> Result<u16, String> {
    match s {
        "100" => Ok(100),
        "200" => Ok(200),
        _ => Err(format!("--target must be 100 or 200, got {s}")),
    }
}

/// Plays one game of uniformly random canonical legal moves and returns the
/// move log. Round r's deal uses the stream derive(seed, r); moves use
/// derive(seed, 0).
fn selfplay(mode: Mode, seed: u64, target: u16) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "selfplay mode {} seed {seed} target {target}",
        mode_name(mode)
    );
    let mut moves_rng = Rng::new(derive(seed, 0));
    let mut round = 1u64;
    let deal = Deal::shuffled(&mut Rng::new(derive(seed, round)));
    write_deal(&mut out, mode, round, &deal);
    let (mut state, opening) = State::first_round(mode, target, &deal);
    let _ = writeln!(out, "{} auto {opening}", mode.seat_name(state.starter));
    let mut moves = MoveList::new();
    let mut count = 0u32;
    loop {
        match state.phase {
            Phase::Finished => break,
            Phase::RoundOver => {
                round += 1;
                let deal = Deal::shuffled(&mut Rng::new(derive(seed, round)));
                write_deal(&mut out, mode, round, &deal);
                state = state.next_round(&deal);
                let _ = writeln!(out, "{} leads", mode.seat_name(state.starter));
            }
            Phase::Playing => {
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[moves_rng.below(moves.len() as u32) as usize];
                let seat = state.turn;
                let pile_before = state.boneyard().len();
                let event = state.play(mv).event;
                count += 1;
                let mut line = format!("{} ", mode.seat_name(seat));
                match mv {
                    Move::Play { tile, end } => {
                        let side = match end {
                            End::Left => "left",
                            End::Right => "right",
                        };
                        let _ = write!(line, "play {tile} {side}");
                    }
                    Move::Draw => {
                        let _ = write!(line, "draw {}", pile_before - state.boneyard().len());
                    }
                    Move::Pass => line.push_str("pass"),
                }
                if let Some(e) = event {
                    line.push_str(" | ");
                    write_event(&mut line, mode, &e);
                    let _ = write!(
                        line,
                        " | scores {} {} {} {}",
                        mode.team_name(0),
                        state.scores[0],
                        mode.team_name(1),
                        state.scores[1]
                    );
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    let winner = if state.scores[0] >= target { 0 } else { 1 };
    let _ = writeln!(
        out,
        "final {} {} {} {} winner {} rounds {round} moves {count}",
        mode.team_name(0),
        state.scores[0],
        mode.team_name(1),
        state.scores[1],
        mode.team_name(winner)
    );
    out
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::OneVsOne => "1v1",
        Mode::TwoVsTwo => "2v2",
    }
}

fn write_deal(out: &mut String, mode: Mode, round: u64, deal: &Deal) {
    let _ = write!(out, "round {round} deal");
    for seat in 0..mode.seats() {
        let _ = write!(out, " {}", mode.seat_name(seat));
        for t in deal.hand(seat) {
            let _ = write!(out, " {t}");
        }
    }
    let pile = deal.boneyard(mode);
    if !pile.is_empty() {
        out.push_str(" boneyard");
        for t in pile {
            let _ = write!(out, " {t}");
        }
    }
    out.push('\n');
}

fn write_event(out: &mut String, mode: Mode, event: &Event) {
    let seat = |s: u8| mode.seat_name(s);
    let _ = match *event {
        Event::PaseSalida { team } => write!(out, "pase de salida +25 {}", mode.team_name(team)),
        Event::PaseCorrido { team } => write!(out, "pase corrido +25 {}", mode.team_name(team)),
        Event::Domino { seat: s, points } => write!(out, "domino {} +{points}", seat(s)),
        Event::Capicua { seat: s, points } => write!(out, "capicua {} +{points}", seat(s)),
        Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        } => write!(
            out,
            "tranque blocker {} {blocker_pips} rival {} {rival_pips} winner {} +{points}",
            seat(blocker),
            seat(rival),
            seat(winner)
        ),
    };
}

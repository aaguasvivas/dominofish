//! The dominofish command line.

mod capi_fuzz;
mod engine;
mod harness;
mod stats;

use std::fmt::Write as _;
use std::process::ExitCode;

use lexopt::ValueExt;

use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, Phase, State};

const USAGE: &str = "usage: dominofish selfplay --mode 1v1|2v2 --seed N [--target 100|200]
       dominofish capi-fuzz --mode 1v1|2v2 --rounds N --seed N
       dominofish playout-bench --mode 1v1|2v2 --rounds N --seed N
       dominofish engine --bot random|heaviest|accompanied|rule
       dominofish match --mode 1v1|2v2 --unit round|game --target 100|200 --pairs N --seed N --a CMD --b CMD --out FILE [--timeout-ms N]";

fn main() -> ExitCode {
    match run() {
        Ok((out, passed)) => {
            print!("{out}");
            if passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("ERROR {e}");
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The output to print and whether the command passed.
fn run() -> Result<(String, bool), String> {
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
            Ok((selfplay(mode, seed, target), true))
        }
        "playout-bench" => {
            let (mut mode, mut rounds, mut seed) = (None, None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("rounds") => rounds = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            let mode = mode.ok_or("--mode is required")?;
            let rounds = u32::try_from(rounds.ok_or("--rounds is required")?)
                .map_err(|_| "--rounds is too large")?;
            let seed = u32::try_from(seed.ok_or("--seed is required")?)
                .map_err(|_| "--seed must fit in 32 bits")?;
            let start = std::time::Instant::now();
            let (checksum, plies) = dominofish_core::bench::playouts(mode, rounds, u64::from(seed));
            let secs = start.elapsed().as_secs_f64();
            Ok((
                format!(
                    "native playouts: mode {} rounds {rounds} seed {seed} checksum {checksum:x} plies {plies} seconds {secs:.3} rounds/s {:.0}\n",
                    mode_name(mode),
                    f64::from(rounds) / secs
                ),
                true,
            ))
        }
        "engine" => {
            let mut bot = None;
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("bot") => {
                        let name = value(&mut parser)?;
                        bot = Some(
                            dominofish_core::bots::Bot::from_name(&name)
                                .ok_or(format!("unknown bot {name}"))?,
                        );
                    }
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            engine::run(bot.ok_or("--bot is required")?)?;
            Ok((String::new(), true))
        }
        "match" => {
            let (mut mode, mut unit, mut target, mut pairs, mut seed) =
                (None, None, None, None, None);
            let (mut a, mut b, mut out, mut timeout_ms) = (None, None, None, 10_000u64);
            let mut sprt: Option<stats::Sprt> = None;
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("unit") => {
                        unit = Some(match value(&mut parser)?.as_str() {
                            "round" => harness::Unit::Round,
                            "game" => harness::Unit::Game,
                            u => return Err(format!("--unit must be round or game, got {u}")),
                        })
                    }
                    Long("target") => target = Some(parse_target(&value(&mut parser)?)?),
                    Long("pairs") => pairs = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("a") => a = Some(harness::Side::new(&value(&mut parser)?)?),
                    Long("b") => b = Some(harness::Side::new(&value(&mut parser)?)?),
                    Long("out") => out = Some(value(&mut parser)?),
                    Long("timeout-ms") => timeout_ms = parse_num(&value(&mut parser)?)?,
                    Long("sprt") => {
                        let v = value(&mut parser)?;
                        let (h0, h1) = v.split_once(',').ok_or("--sprt takes mu0,mu1")?;
                        let parse = |x: &str| {
                            x.parse::<f64>()
                                .map_err(|_| format!("bad --sprt bound {x}"))
                        };
                        let (h0, h1) = (parse(h0)?, parse(h1)?);
                        if h1 <= h0 {
                            return Err("--sprt needs mu0 < mu1".into());
                        }
                        sprt = Some(stats::Sprt::new(h0, h1, 0.05, 0.05));
                    }
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            let spec = harness::Spec {
                mode: mode.ok_or("--mode is required")?,
                unit: unit.ok_or("--unit is required")?,
                target: target.ok_or("--target is required")?,
                pairs: pairs.ok_or("--pairs is required")?,
                seed: seed.ok_or("--seed is required")?,
                a: a.ok_or("--a is required")?,
                b: b.ok_or("--b is required")?,
                timeout: std::time::Duration::from_millis(timeout_ms),
            };
            let out = out.ok_or("--out is required")?;
            // The SPRT runs on each pair's margin and stops the match at a
            // decision; --pairs is then the maximum.
            let report = harness::run(&spec, &mut |pair| {
                sprt.as_mut().is_some_and(|t| {
                    t.push(f64::from(pair.margin()));
                    t.decision().is_some()
                })
            });
            Ok(write_match(&spec, &report, sprt.as_ref(), &out)?)
        }
        "capi-fuzz" => {
            let (mut mode, mut rounds, mut seed) = (None, None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("rounds") => rounds = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            let outcome = capi_fuzz::run(
                mode.ok_or("--mode is required")?,
                rounds.ok_or("--rounds is required")?,
                seed.ok_or("--seed is required")?,
            )?;
            let mut out = String::new();
            for e in &outcome.errors {
                out.push_str(e);
                out.push('\n');
            }
            out.push_str(&outcome.summary);
            out.push('\n');
            Ok((out, outcome.passed))
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

/// Writes a match's per-pair results to `out` and returns the summary lines
/// (docs/FORMATS.md, Match results) and whether the match had no fault.
fn write_match(
    spec: &harness::Spec,
    report: &harness::Report,
    sprt: Option<&stats::Sprt>,
    out: &str,
) -> Result<(String, bool), String> {
    let mut text = format!("# header {}\n", report.header);
    for p in &report.pairs {
        let _ = writeln!(
            text,
            "pair {} seed {} game1 {}-{} game2 {}-{} rounds {},{} margin {} wins {}",
            p.pair,
            p.deal_seed,
            p.scores[0][0],
            p.scores[0][1],
            p.scores[1][0],
            p.scores[1][1],
            p.rounds[0],
            p.rounds[1],
            p.margin(),
            p.wins(spec.target)
        );
    }
    let per_round: Vec<f64> = report.pairs.iter().map(|p| p.margin_per_round()).collect();
    let r = stats::summary(&per_round);
    let mut lines = format!(
        "points per round (A minus B) mean {:.3} sd {:.3} 95% CI [{:.3}, {:.3}] over {} pairs",
        r.mean, r.sd, r.lo, r.hi, r.n
    );
    if spec.unit == harness::Unit::Game {
        let wins: Vec<f64> = report
            .pairs
            .iter()
            .map(|p| f64::from(p.wins(spec.target)) / 2.0)
            .collect();
        let w = stats::summary(&wins);
        let _ = write!(
            lines,
            "; game-win rate of A {:.3} 95% CI [{:.3}, {:.3}]",
            w.mean, w.lo, w.hi
        );
    }
    if let Some(t) = sprt {
        let (lower, upper) = t.bounds();
        let outcome = match t.decision() {
            Some(stats::Decision::H0) => "H0",
            Some(stats::Decision::H1) => "H1",
            None => "inconclusive",
        };
        let _ = write!(
            lines,
            "; sprt on the pair margin, H0 {} H1 {}, alpha 0.05 beta 0.05: {outcome} after {} pairs, llr {:.3} in [{:.3}, {:.3}]",
            t.mu0,
            t.mu1,
            t.n(),
            t.llr(),
            lower,
            upper
        );
    }
    let _ = writeln!(text, "# summary {lines}");
    if let Some((kind, detail)) = &report.fault {
        let _ = writeln!(text, "# fault {kind:?}: {detail}");
    }
    std::fs::write(out, text).map_err(|e| format!("{out}: {e}"))?;
    let summary = match &report.fault {
        None => format!(
            "{lines}\nmatch: PASS mode {} unit {} target {} pairs {} moves {}; illegal 0, crashes 0, timeouts 0; results {out}\n",
            mode_name(spec.mode),
            if spec.unit == harness::Unit::Round {
                "round"
            } else {
                "game"
            },
            spec.target,
            report.pairs.len(),
            report.moves,
        ),
        Some((kind, detail)) => format!(
            "ERROR {kind:?}: {detail}\nmatch: FAIL {kind:?} after {} pairs; results {out}\n",
            report.pairs.len()
        ),
    };
    Ok((summary, report.fault.is_none()))
}

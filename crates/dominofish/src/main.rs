//! The dominofish command line.

mod capi_fuzz;
mod engine;
mod harness;
mod help;
mod report;
mod selfplay;
mod stats;

use std::fmt::Write as _;
use std::process::ExitCode;

use lexopt::ValueExt;

use dominofish_core::rules::Mode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    match first {
        None => {
            eprint!("{}", help::overview());
            return ExitCode::from(2);
        }
        Some("help" | "--help" | "-h") => {
            match args.get(1).and_then(|c| help::find(c)) {
                Some(c) => print!("{}", help::command(c)),
                None => print!("{}", help::overview()),
            }
            return ExitCode::SUCCESS;
        }
        Some("version" | "--version" | "-V") => {
            println!("DominoFish {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some(cmd) if args.iter().any(|a| a == "--help" || a == "-h") => {
            if let Some(c) = help::find(cmd) {
                print!("{}", help::command(c));
                return ExitCode::SUCCESS;
            }
        }
        _ => {}
    }
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
            match first.and_then(help::find) {
                Some(c) => eprintln!(
                    "\nUsage: {}\nRun \"dominofish {} --help\" for its options.",
                    c.usage, c.name
                ),
                None => eprint!("\n{}", help::overview()),
            }
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
            Ok((selfplay::run(mode, seed, target), true))
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
            let (mut bot, mut salt) = (None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("bot") => {
                        let name = value(&mut parser)?;
                        bot = Some(
                            dominofish_core::bots::Bot::from_name(&name)
                                .ok_or(format!("unknown bot {name}"))?,
                        );
                    }
                    Long("salt") => salt = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            engine::run(bot.ok_or("--bot is required")?, salt)?;
            Ok((String::new(), true))
        }
        "match" => {
            let (mut mode, mut unit, mut target, mut pairs, mut seed) =
                (None, None, None, None, None);
            let (mut a, mut b, mut out, mut timeout_ms) = (None, None, None, 10_000u64);
            let mut engine_seed = None;
            let mut sprt: Option<stats::Sprt> = None;
            let mut track = false;
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
                    Long("engine-seed") => engine_seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("a") => a = Some(harness::Side::new(&value(&mut parser)?)?),
                    Long("b") => b = Some(harness::Side::new(&value(&mut parser)?)?),
                    Long("out") => out = Some(value(&mut parser)?),
                    Long("timeout-ms") => timeout_ms = parse_num(&value(&mut parser)?)?,
                    Long("sprt-track") => track = true,
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
                engine_seed: engine_seed.ok_or("--engine-seed is required")?,
            };
            let out = out.ok_or("--out is required")?;
            // The SPRT runs on each pair's margin and stops the match at a
            // decision; --pairs is then the maximum. With --sprt-track the
            // match plays every pair and the outcome is the first decision.
            let report = harness::run(&spec, &mut |pair| {
                sprt.as_mut().is_some_and(|t| {
                    t.push(f64::from(pair.margin()));
                    !track && t.decision().is_some()
                })
            });
            Ok(write_match(&spec, &report, sprt.as_ref(), &out)?)
        }
        "report" => {
            let (mut results, mut out, mut check) = (
                "docs/eval/results".to_string(),
                "docs/eval/REPORT.md".to_string(),
                false,
            );
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("results") => results = value(&mut parser)?,
                    Long("out") => out = value(&mut parser)?,
                    Long("check") => check = true,
                    _ => return Err(arg.unexpected().to_string()),
                }
            }
            Ok(report::run(
                std::path::Path::new(&results),
                std::path::Path::new(&out),
                check,
            )?)
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

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::OneVsOne => "1v1",
        Mode::TwoVsTwo => "2v2",
    }
}

/// Writes a match's per-pair results to `out` and returns the summary lines
/// (docs/FORMATS.md, Match results) and whether the match had no fault.
fn write_match(
    spec: &harness::Spec,
    report: &harness::Report,
    sprt: Option<&stats::Sprt>,
    out: &str,
) -> Result<(String, bool), String> {
    let mut text = format!(
        "# header {}\n# seeds deals={}..{} engines={}\n",
        report.header,
        spec.seed,
        spec.seed + spec.pairs.saturating_sub(1),
        spec.engine_seed
    );
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
        let (outcome, at) = match t.first_decision() {
            Some((stats::Decision::H0, n)) => ("H0", n),
            Some((stats::Decision::H1, n)) => ("H1", n),
            None => ("inconclusive", t.n()),
        };
        let _ = write!(
            lines,
            "; sprt on the pair margin, H0 {} H1 {}, alpha 0.05 beta 0.05: {outcome} after {at} pairs, llr {:.3} in [{:.3}, {:.3}] after {} pairs",
            t.mu0,
            t.mu1,
            t.llr(),
            lower,
            upper,
            t.n()
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

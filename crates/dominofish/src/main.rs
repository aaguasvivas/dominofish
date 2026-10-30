//! The dominofish command line.

mod analyze;
mod calibrate;
mod capi_fuzz;
mod engine;
mod equity;
mod harness;
mod help;
mod report;
mod selfplay;
mod solve;
mod stats;
mod world_stats;

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
        Err(Failure::Run(e)) => {
            eprintln!("ERROR {e}");
            ExitCode::from(1)
        }
        Err(Failure::Usage(e)) => {
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

/// Why a command failed: a usage error (exit 2, with the command's usage)
/// or an error while it ran (exit 1).
enum Failure {
    Usage(String),
    Run(String),
}

impl From<String> for Failure {
    fn from(e: String) -> Failure {
        Failure::Usage(e)
    }
}

impl From<&str> for Failure {
    fn from(e: &str) -> Failure {
        Failure::Usage(e.into())
    }
}

/// The output to print and whether the command passed.
fn run() -> Result<(String, bool), Failure> {
    use lexopt::prelude::*;
    let mut parser = lexopt::Parser::from_env();
    let command = match parser.next().map_err(|e| e.to_string())? {
        Some(Value(v)) => v.string().map_err(|e| e.to_string())?,
        _ => return Err("missing command".into()),
    };
    match command.as_str() {
        "selfplay" => {
            let (mut mode, mut seed, mut target, mut record) = (None, None, 100u16, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("target") => target = parse_target(&value(&mut parser)?)?,
                    Long("record") => record = Some(value(&mut parser)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let mode = mode.ok_or("--mode is required")?;
            let seed = seed.ok_or("--seed is required")?;
            let (log, game) = selfplay::run(mode, seed, target);
            if let Some(path) = record {
                std::fs::write(&path, game.to_text())
                    .map_err(|e| Failure::Run(format!("{path}: {e}")))?;
            }
            Ok((log, true))
        }
        "pimc-stats" => {
            let (mut mode, mut games, mut seed, mut worlds) = (None, None, None, None);
            let (mut nodes, mut bits) = (solve::NODE_CAP, solve::PIMC_TABLE_BITS);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("games") => games = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("worlds") => worlds = Some(parse_worlds(&value(&mut parser)?)?),
                    Long("nodes") => nodes = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let mode = mode.ok_or("--mode is required")?;
            let games = games.ok_or("--games is required")?;
            if games == 0 {
                return Err("--games must be at least 1".into());
            }
            let seed = seed.ok_or("--seed is required")?;
            let limits = dominofish_core::pimc::Limits {
                worlds: worlds.unwrap_or(solve::PIMC_WORLDS),
                nodes,
                table_bits: bits,
            };
            Ok((solve::pimc_stats(mode, games, seed, &limits), true))
        }
        "equity" => {
            let (mut mode, mut rounds, mut seed, mut engine_seed) = (None, None, None, None);
            let (mut from, mut out_dir, mut check) = (None, None, None);
            let (mut threads, mut worlds) = (1u64, solve::PIMC_WORLDS);
            let (mut nodes, mut bits) = (solve::NODE_CAP, solve::PIMC_TABLE_BITS);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("rounds") => rounds = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("engine-seed") => engine_seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("threads") => threads = parse_num(&value(&mut parser)?)?,
                    Long("worlds") => worlds = parse_worlds(&value(&mut parser)?)?,
                    Long("nodes") => nodes = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    Long("from") => from = Some(value(&mut parser)?),
                    Long("out-dir") => out_dir = Some(value(&mut parser)?),
                    Long("check") => check = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let out_dir = std::path::PathBuf::from(out_dir.ok_or("--out-dir is required")?);
            if !(1..=4).contains(&threads) {
                return Err(format!("--threads must be 1 to 4, got {threads}").into());
            }
            std::fs::create_dir_all(&out_dir)
                .map_err(|e| Failure::Run(format!("{}: {e}", out_dir.display())))?;
            let write = |name: &str, text: &str| {
                let path = out_dir.join(name);
                std::fs::write(&path, text)
                    .map_err(|e| Failure::Run(format!("{}: {e}", path.display())))
            };
            let mut log = String::new();
            let (mode, outcomes, rounds_name) = match from {
                Some(path) => {
                    let text = std::fs::read_to_string(&path)
                        .map_err(|e| Failure::Run(format!("{path}: {e}")))?;
                    let (mode, outcomes) =
                        equity::parse_rounds(&text).map_err(|e| format!("{path}: {e}"))?;
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map_or(path.clone(), |n| n.to_string_lossy().to_string());
                    (mode, outcomes, name)
                }
                None => {
                    let mode = mode.ok_or("--mode is required without --from")?;
                    let wanted = rounds.ok_or("--rounds is required without --from")?;
                    if wanted < 2 {
                        return Err("--rounds must be at least 2".into());
                    }
                    let seed = seed.ok_or("--seed is required without --from")?;
                    let engine_seed =
                        engine_seed.ok_or("--engine-seed is required without --from")?;
                    let limits = dominofish_core::pimc::Limits {
                        worlds,
                        nodes,
                        table_bits: bits,
                    };
                    let start = std::time::Instant::now();
                    let (recorded, work) = equity::record(
                        mode,
                        wanted as usize,
                        seed,
                        engine_seed,
                        &limits,
                        threads as usize,
                    )?;
                    let settings = format!(
                        "deals={seed}+game engines={engine_seed} worlds={worlds} nodes={nodes} table_bits={bits}"
                    );
                    let name = format!("rounds-{}.txt", mode.name());
                    write(
                        &name,
                        &equity::rounds_text(mode, &settings, &recorded, &work),
                    )?;
                    let _ = writeln!(
                        log,
                        "equity: {} rounds after the first from {} games ({} moves, {} searched decisions, {} capped solves) in {:.1} s on {threads} threads; wrote {}",
                        recorded.len(),
                        work.games,
                        work.moves,
                        work.decisions,
                        work.capped,
                        start.elapsed().as_secs_f64(),
                        out_dir.join(&name).display()
                    );
                    (
                        mode,
                        recorded.into_iter().map(|r| r.outcome).collect(),
                        name,
                    )
                }
            };
            for target in equity::TARGETS {
                let w = dominofish_core::equity::Equity::build(target, &outcomes);
                let name = format!("w-{}-{target}.txt", mode.name());
                write(
                    &name,
                    &equity::table_text(mode, &w, &outcomes, &rounds_name),
                )?;
                let _ = writeln!(log, "equity: wrote {}", out_dir.join(&name).display());
                if let Some(games) = check {
                    for (k, start) in equity::check_starts(target).into_iter().enumerate() {
                        let (share, p, z) =
                            equity::simulate(&w, &outcomes, start, games as u32, 1000 + k as u64);
                        let _ = writeln!(
                            log,
                            "equity check: target {target} start {}-{} {} W {p:.4} simulated {share:.4} over {games} games z {z:.2}",
                            start.0,
                            start.1,
                            if start.2 { "lead" } else { "other" }
                        );
                    }
                }
            }
            Ok((log, true))
        }
        "calibrate" => {
            let (mut mode, mut target, mut games, mut seed, mut engine_seed) =
                (None, None, None, None, None);
            let (mut from, mut out, mut equity_dir) = (None, None, None);
            let (mut threads, mut worlds) = (1u64, solve::PIMC_WORLDS);
            let (mut nodes, mut bits) = (solve::NODE_CAP, solve::PIMC_TABLE_BITS);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("target") => target = Some(parse_num(&value(&mut parser)?)?),
                    Long("games") => games = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("engine-seed") => engine_seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("threads") => threads = parse_num(&value(&mut parser)?)?,
                    Long("worlds") => worlds = parse_worlds(&value(&mut parser)?)?,
                    Long("nodes") => nodes = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    Long("equity") => equity_dir = Some(value(&mut parser)?),
                    Long("from") => from = Some(value(&mut parser)?),
                    Long("out") => out = Some(value(&mut parser)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            if let Some(path) = from {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| Failure::Run(format!("{path}: {e}")))?;
                let games = calibrate::parse(&text).map_err(|e| format!("{path}: {e}"))?;
                return Ok((calibrate::summary(&games), true));
            }
            let mode = mode.ok_or("--mode is required without --from")?;
            let target = match target.ok_or("--target is required without --from")? {
                100 => 100u16,
                200 => 200,
                t => return Err(format!("--target must be 100 or 200, got {t}").into()),
            };
            let games = games.ok_or("--games is required without --from")?;
            if games == 0 {
                return Err("--games must be at least 1".into());
            }
            let seed = seed.ok_or("--seed is required without --from")?;
            let engine_seed = engine_seed.ok_or("--engine-seed is required without --from")?;
            let out = out.ok_or("--out is required without --from")?;
            if !(1..=4).contains(&threads) {
                return Err(format!("--threads must be 1 to 4, got {threads}").into());
            }
            let dir = std::path::Path::new(equity_dir.as_deref().unwrap_or(EQUITY_DIR));
            let tables = equity::load_tables(dir)?;
            let table_hash =
                equity::file_hash(&dir.join(format!("w-{}-{target}.txt", mode.name())))?;
            let w = tables
                .get(mode, target)
                .ok_or_else(|| format!("no W table for {} target {target}", mode.name()))?;
            let limits = dominofish_core::pimc::Limits {
                worlds,
                nodes,
                table_bits: bits,
            };
            let start = std::time::Instant::now();
            let (played, work) = calibrate::run(
                mode,
                target,
                games,
                seed,
                engine_seed,
                &limits,
                w,
                threads as usize,
            )?;
            let settings = format!(
                "deals={seed}+game engines={engine_seed} worlds={worlds} nodes={nodes} table_bits={bits} table={table_hash:016x}"
            );
            let text = calibrate::text(mode, target, &settings, &played, &work);
            std::fs::write(&out, &text).map_err(|e| Failure::Run(format!("{out}: {e}")))?;
            let mut log = format!(
                "calibrate: {games} games ({} moves, {} searched decisions, {} capped solves) in {:.1} s on {threads} threads; wrote {out}\n",
                work.moves,
                work.decisions,
                work.capped,
                start.elapsed().as_secs_f64()
            );
            log.push_str(&calibrate::summary(&played));
            Ok((log, true))
        }
        "world-stats" => {
            let (mut mode, mut games, mut seed) = (None, None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("games") => games = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let mode = mode.ok_or("--mode is required")?;
            let games = games.ok_or("--games is required")?;
            if games == 0 {
                return Err("--games must be at least 1".into());
            }
            let seed = seed.ok_or("--seed is required")?;
            Ok((world_stats::run(mode, games, seed), true))
        }
        "analyze" => {
            let (mut file, mut round, mut ply, mut worlds, mut seed) =
                (None, None, None, None, 1u64);
            let (mut nodes, mut bits) = (solve::NODE_CAP, solve::PIMC_TABLE_BITS);
            let (mut win, mut equity_dir) = (false, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("record") => file = Some(value(&mut parser)?),
                    Long("objective") => win = parse_objective(&value(&mut parser)?)?,
                    Long("equity") => equity_dir = Some(value(&mut parser)?),
                    Long("round") => round = Some(parse_num(&value(&mut parser)?)?),
                    Long("ply") => ply = Some(parse_num(&value(&mut parser)?)?),
                    Long("worlds") => worlds = Some(parse_worlds(&value(&mut parser)?)?),
                    Long("seed") => seed = parse_num(&value(&mut parser)?)?,
                    Long("nodes") => nodes = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let file = file.ok_or("--record is required")?;
            let round = round.ok_or("--round is required")?;
            let ply = ply.ok_or("--ply is required")?;
            let limits = dominofish_core::pimc::Limits {
                worlds: worlds.unwrap_or(solve::PIMC_WORLDS),
                nodes,
                table_bits: bits,
            };
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
            let tables = if win {
                Some(equity::load_tables(std::path::Path::new(
                    equity_dir.as_deref().unwrap_or(EQUITY_DIR),
                ))?)
            } else if equity_dir.is_some() {
                return Err("--equity applies to --objective win only"
                    .to_string()
                    .into());
            } else {
                None
            };
            Ok((
                analyze::run(&text, round, ply, &limits, seed, tables.as_ref())?,
                true,
            ))
        }
        "playout-bench" => {
            let (mut mode, mut rounds, mut seed) = (None, None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("rounds") => rounds = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
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
                    mode.name(),
                    f64::from(rounds) / secs
                ),
                true,
            ))
        }
        "engine" => {
            let (mut bot, mut salt, mut worlds) = (None, None, None);
            let (mut nodes, mut bits) = (solve::NODE_CAP, None);
            let (mut win, mut equity_dir) = (false, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("bot") => bot = Some(value(&mut parser)?),
                    Long("objective") => win = parse_objective(&value(&mut parser)?)?,
                    Long("equity") => equity_dir = Some(value(&mut parser)?),
                    Long("salt") => salt = Some(parse_num(&value(&mut parser)?)?),
                    Long("nodes") => nodes = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = Some(parse_bits(&value(&mut parser)?)?),
                    Long("worlds") => worlds = Some(parse_worlds(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let name = bot.ok_or("--bot is required")?;
            if (worlds.is_some() || win || equity_dir.is_some()) && name != "pimc" {
                return Err(
                    "--worlds, --objective, and --equity apply to --bot pimc only"
                        .to_string()
                        .into(),
                );
            }
            let player = if name == "omniscient" {
                engine::Player::Omniscient {
                    nodes,
                    bits: bits.unwrap_or(solve::TABLE_BITS),
                }
            } else if name == "pimc" {
                let tables = if win {
                    Some(equity::load_tables(std::path::Path::new(
                        equity_dir.as_deref().unwrap_or(EQUITY_DIR),
                    ))?)
                } else {
                    None
                };
                engine::Player::Pimc(
                    dominofish_core::pimc::Limits {
                        worlds: worlds.unwrap_or(solve::PIMC_WORLDS),
                        nodes,
                        table_bits: bits.unwrap_or(solve::PIMC_TABLE_BITS),
                    },
                    tables,
                )
            } else {
                engine::Player::Bot(
                    dominofish_core::bots::Bot::from_name(&name)
                        .ok_or(format!("unknown bot {name}"))?,
                )
            };
            engine::run(player, salt).map_err(Failure::Run)?;
            Ok((String::new(), true))
        }
        "match" => {
            let (mut mode, mut unit, mut target, mut pairs, mut seed) =
                (None, None, None, None, None);
            let (mut a, mut b, mut out, mut timeout_ms) = (None, None, None, 10_000u64);
            let mut engine_seed = None;
            let mut sprt: Option<stats::Sprt> = None;
            let mut sprt_wins = false;
            let mut track = false;
            let mut aivat = None;
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("unit") => {
                        unit = Some(match value(&mut parser)?.as_str() {
                            "round" => harness::Unit::Round,
                            "game" => harness::Unit::Game,
                            u => {
                                return Err(format!("--unit must be round or game, got {u}").into());
                            }
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
                    Long("aivat") => {
                        let n = parse_num(&value(&mut parser)?)?;
                        if !(1..=64).contains(&n) {
                            return Err(format!("--aivat must be 1 to 64, got {n}").into());
                        }
                        aivat = Some(n as u32);
                    }
                    Long("sprt-track") => track = true,
                    Long("sprt-on") => {
                        sprt_wins = match value(&mut parser)?.as_str() {
                            "margin" => false,
                            "wins" => true,
                            x => {
                                return Err(
                                    format!("--sprt-on must be margin or wins, got {x}").into()
                                );
                            }
                        }
                    }
                    Long("sprt") => {
                        let v = value(&mut parser)?;
                        let (h0, h1) = v.split_once(',').ok_or("--sprt takes mu0,mu1")?;
                        let parse = |x: &str| {
                            x.parse::<f64>()
                                .map_err(|_| format!("bad --sprt bound {x}"))
                        };
                        let (h0, h1) = (parse(h0)?, parse(h1)?);
                        if !(h0.is_finite() && h1.is_finite() && h1 > h0) {
                            return Err("--sprt needs finite mu0 < mu1".into());
                        }
                        sprt = Some(stats::Sprt::new(h0, h1, 0.05, 0.05));
                    }
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let spec = harness::Spec {
                mode: mode.ok_or("--mode is required")?,
                unit: unit.ok_or("--unit is required")?,
                target: target.ok_or("--target is required")?,
                pairs: match pairs.ok_or("--pairs is required")? {
                    0 => return Err("--pairs must be at least 1".into()),
                    n => n,
                },
                seed: seed.ok_or("--seed is required")?,
                a: a.ok_or("--a is required")?,
                b: b.ok_or("--b is required")?,
                timeout: std::time::Duration::from_millis(timeout_ms),
                engine_seed: engine_seed.ok_or("--engine-seed is required")?,
                aivat,
            };
            let out = out.ok_or("--out is required")?;
            if sprt_wins && (sprt.is_none() || spec.unit != harness::Unit::Game) {
                return Err("--sprt-on wins needs --sprt and --unit game".into());
            }
            // The SPRT runs on each pair's margin, or with --sprt-on wins on
            // its share of games won by A, and stops the match at a
            // decision; --pairs is then the maximum. With --sprt-track the
            // match plays every pair and the outcome is the first decision.
            let target = spec.target;
            let report = harness::run(&spec, &mut |pair| {
                sprt.as_mut().is_some_and(|t| {
                    t.push(if sprt_wins {
                        f64::from(pair.wins(target)) / 2.0
                    } else {
                        f64::from(pair.margin())
                    });
                    !track && t.decision().is_some()
                })
            });
            write_match(&spec, &report, sprt.as_ref(), sprt_wins, &out).map_err(Failure::Run)
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
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            report::run(
                std::path::Path::new(&results),
                std::path::Path::new(&out),
                check,
            )
            .map_err(Failure::Run)
        }
        "capi-fuzz" => {
            let (mut mode, mut rounds, mut seed) = (None, None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("rounds") => rounds = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            let outcome = capi_fuzz::run(
                mode.ok_or("--mode is required")?,
                rounds.ok_or("--rounds is required")?,
                seed.ok_or("--seed is required")?,
            )
            .map_err(Failure::Run)?;
            let mut out = String::new();
            for e in &outcome.errors {
                out.push_str(e);
                out.push('\n');
            }
            out.push_str(&outcome.summary);
            out.push('\n');
            Ok((out, outcome.passed))
        }
        "bench" => {
            let (mut file, mut cap, mut bits) = (None, solve::NODE_CAP, solve::TABLE_BITS);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("positions") => file = Some(value(&mut parser)?),
                    Long("nodes") => cap = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            Ok((
                solve::bench(file.as_deref(), cap, bits).map_err(Failure::Run)?,
                true,
            ))
        }
        "solve-stats" => {
            let (mut mode, mut games, mut seed) = (None, None, None);
            let (mut cap, mut bits) = (solve::NODE_CAP, solve::TABLE_BITS);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("mode") => mode = Some(parse_mode(&value(&mut parser)?)?),
                    Long("games") => games = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    Long("nodes") => cap = parse_num(&value(&mut parser)?)?,
                    Long("table-bits") => bits = parse_bits(&value(&mut parser)?)?,
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            Ok((
                solve::stats(
                    mode.ok_or("--mode is required")?,
                    games.ok_or("--games is required")?,
                    seed.ok_or("--seed is required")?,
                    cap,
                    bits,
                ),
                true,
            ))
        }
        "bench-positions" => {
            let (mut games, mut seed) = (None, None);
            while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
                match arg {
                    Long("games") => games = Some(parse_num(&value(&mut parser)?)?),
                    Long("seed") => seed = Some(parse_num(&value(&mut parser)?)?),
                    _ => return Err(arg.unexpected().to_string().into()),
                }
            }
            Ok((
                solve::bench_positions(
                    games.ok_or("--games is required")?,
                    seed.ok_or("--seed is required")?,
                ),
                true,
            ))
        }
        other => Err(format!("unknown command {other}").into()),
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

/// A world count for PIMC: 1 to 4,096.
fn parse_worlds(s: &str) -> Result<u32, String> {
    let n = parse_num(s)?;
    if (1..=4096).contains(&n) {
        Ok(n as u32)
    } else {
        Err(format!("--worlds must be 1 to 4096, got {s}"))
    }
}

/// The committed W tables (docs/PLAN.md D20), read from the working
/// directory, the repository root.
const EQUITY_DIR: &str = "docs/eval/equity";

/// --objective points|win: whether PIMC plays for the win (D21).
fn parse_objective(s: &str) -> Result<bool, String> {
    match s {
        "points" => Ok(false),
        "win" => Ok(true),
        _ => Err(format!("--objective must be points or win, got {s}")),
    }
}

fn parse_bits(s: &str) -> Result<u32, String> {
    let bits = parse_num(s)?;
    if (10..=u64::from(solve::MAX_TABLE_BITS)).contains(&bits) {
        Ok(bits as u32)
    } else {
        Err(format!(
            "--table-bits must be 10 to {} (at most 256 MB), got {s}",
            solve::MAX_TABLE_BITS
        ))
    }
}

fn parse_target(s: &str) -> Result<u16, String> {
    match s {
        "100" => Ok(100),
        "200" => Ok(200),
        _ => Err(format!("--target must be 100 or 200, got {s}")),
    }
}

/// Writes a match's per-pair results to `out` and returns the summary lines
/// (docs/FORMATS.md, Match results) and whether the match had no fault.
fn write_match(
    spec: &harness::Spec,
    report: &harness::Report,
    sprt: Option<&stats::Sprt>,
    sprt_wins: bool,
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
    for p in &report.pairs {
        if let Some(a) = p.aivat {
            let _ = writeln!(
                text,
                "aivat {} deal {:.3} draw {:.3} action {:.3}",
                p.pair, a.deal, a.draw, a.action
            );
        }
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
        let (w_lo, w_hi) = stats::wilson(w.mean * 2.0 * w.n as f64, 2.0 * w.n as f64);
        let _ = write!(
            lines,
            "; game-win rate of A {:.3} 95% CI [{:.3}, {:.3}]",
            w.mean, w_lo, w_hi
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
            "; sprt on the {}, H0 {} H1 {}, alpha 0.05 beta 0.05: {outcome} after {at} pairs, llr {:.3} in [{:.3}, {:.3}] after {} pairs",
            if sprt_wins {
                "game-win share"
            } else {
                "pair margin"
            },
            t.mu0,
            t.mu1,
            t.llr(),
            lower,
            upper,
            t.n()
        );
    }
    let _ = writeln!(text, "# summary {lines}");
    // The corrected summary has its own line: the summary line stays the
    // raw result, as every run before M5 wrote it.
    let mut aivat_line = String::new();
    if let Some(j) = spec.aivat {
        let rows: Vec<(f64, [f64; 3])> =
            report.pairs.iter().filter_map(|p| p.aivat_row()).collect();
        let (corrected, b) = stats::crossfit(&rows);
        let c = stats::summary(&corrected);
        // Corrected minus raw per pair: its mean is 0 when the correction
        // is unbiased (tests T5, T6, and T7).
        let diff: Vec<f64> = corrected.iter().zip(&rows).map(|(c, r)| c - r.0).collect();
        let d = stats::summary(&diff);
        let se = d.sd / (d.n as f64).sqrt();
        aivat_line = format!(
            "corrected points per round (AIVAT, cross-fitted) mean {:.3} sd {:.3} 95% CI [{:.3}, {:.3}] over {} pairs; raw sd {:.3}; corrected minus raw mean {:.4} se {:.4} t {:.3}; coefficients deal draw action {:.3},{:.3},{:.3} and {:.3},{:.3},{:.3}",
            c.mean,
            c.sd,
            c.lo,
            c.hi,
            c.n,
            r.sd,
            d.mean,
            se,
            if se > 0.0 { d.mean / se } else { 0.0 },
            b[0][0],
            b[0][1],
            b[0][2],
            b[1][0],
            b[1][1],
            b[1][2]
        );
        let actions: Vec<&str> = ["a", "b"]
            .into_iter()
            .zip(report.aivat_actions)
            .filter_map(|(side, on)| on.then_some(side))
            .collect();
        let _ = writeln!(
            text,
            "# aivat j={j} nodes={} values={} capped={} actions={} draws={} {aivat_line}",
            harness::AIVAT_NODES,
            report.aivat_values,
            report.aivat_capped,
            if actions.is_empty() {
                "-".to_string()
            } else {
                actions.join(",")
            },
            if report.aivat_draws { "on" } else { "off" }
        );
        aivat_line.push('\n');
    }
    let _ = writeln!(
        text,
        "# capped a={} b={}",
        report.capped[0], report.capped[1]
    );
    let think = |t: &harness::Think| {
        format!(
            "{:.1},{:.1},{}",
            t.total_ns as f64 / 1e6 / t.moves.max(1) as f64,
            t.max_ns as f64 / 1e6,
            t.moves
        )
    };
    let _ = writeln!(
        text,
        "# time a={} b={}",
        think(&report.think[0]),
        think(&report.think[1])
    );
    if let Some((kind, detail)) = &report.fault {
        let _ = writeln!(text, "# fault {kind:?}: {detail}");
    }
    std::fs::write(out, text).map_err(|e| format!("{out}: {e}"))?;
    let summary = match &report.fault {
        None => format!(
            "{lines}\n{aivat_line}match: PASS mode {} unit {} target {} pairs {} moves {}; illegal 0, crashes 0, timeouts 0; results {out}\n",
            spec.mode.name(),
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

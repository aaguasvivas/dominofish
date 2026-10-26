//! `dominofish engine --bot <name>`: a player speaking the engine protocol
//! (docs/FORMATS.md, Engine protocol) on stdin and stdout.
//!
//! A baseline bot decides from the `infoset` line the referee sends before
//! each `go`, with an RNG drawn from the game's seed and the round, so a
//! seed keyed by deal and seat gives the same choices in both games of a
//! duplicate pair. A salt mixes into that seed, so two copies of one bot can
//! be seeded independently (the M2-08 null test).
//!
//! The omniscient bot (M3-04) cheats on purpose: its ready line says
//! view=omniscient, so the referee also sends it the full state, every
//! hidden tile and the boneyard order included, and it plays the exact
//! solver's move. It is an upper bound for comparison, never a goal, and is
//! labeled as cheating wherever it appears.

use std::io::{BufRead, Write};

use dominofish_core::bots::Bot;
use dominofish_core::infoset::InfoSet;
use dominofish_core::protocol::{FromEngine, ToEngine, VERSION, View};
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Mode, State};
use dominofish_core::solver::{self, Table};

/// Who plays: a baseline bot, or the omniscient bot with its node cap and
/// table size (2^bits entries).
pub enum Player {
    Bot(Bot),
    Omniscient { nodes: u64, bits: u32 },
}

pub fn run(player: Player, salt: Option<u64>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut mode = Mode::TwoVsTwo;
    let mut seed = 0u64;
    let mut rng = Rng::new(0);
    let mut info: Option<InfoSet> = None;
    let mut state: Option<State> = None;
    // The omniscient bot's table; one deal per round, cleared at each.
    let mut table = match player {
        Player::Omniscient { bits, .. } => Some(Table::new(bits)),
        Player::Bot(_) => None,
    };
    let reply = |out: &mut std::io::StdoutLock, msg: FromEngine| -> Result<(), String> {
        writeln!(out, "{}", msg.to_line())
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())
    };
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Some(msg) = ToEngine::parse(&line, mode) else {
            return Err(format!("unreadable line: {line}"));
        };
        match msg {
            ToEngine::Hello { version } if version == VERSION => {
                // A baseline bot has no budgets or tables and runs on one
                // thread; the omniscient bot reports its cap and table.
                let ready = match (&player, &table) {
                    (Player::Omniscient { nodes, .. }, Some(t)) => FromEngine::Ready {
                        version,
                        budgets: format!("nodes:{nodes}"),
                        threads: 1,
                        tables: format!("{}MB", t.bytes() >> 20),
                        view: View::Omniscient,
                    },
                    _ => FromEngine::Ready {
                        version,
                        budgets: "-".into(),
                        threads: 1,
                        tables: "-".into(),
                        view: View::InfoSet,
                    },
                };
                reply(&mut out, ready)?;
            }
            ToEngine::Hello { version } => {
                return Err(format!("protocol version {version} not supported"));
            }
            ToEngine::Header(text) => reply(&mut out, FromEngine::Header(text))?,
            ToEngine::Game {
                mode: m, seed: s, ..
            } => {
                mode = m;
                seed = salt.map_or(s, |x| derive(s, x));
            }
            ToEngine::Round { round, .. } => {
                rng = Rng::new(derive(seed, u64::from(round)));
                if let Some(t) = table.as_mut() {
                    t.clear();
                }
            }
            ToEngine::InfoSet(i) => info = Some(*i),
            ToEngine::State(s) => state = Some(*s),
            ToEngine::Go => {
                let i = info.take().ok_or("go without an infoset")?;
                if i.turn != i.seat {
                    return Err("go for a seat that is not on turn".into());
                }
                let mv = match &player {
                    Player::Bot(bot) => bot.decide(&i, &mut rng),
                    Player::Omniscient { nodes, .. } => {
                        let mut s = state.take().ok_or("go without a state")?;
                        if s.turn != i.seat || s.hands[i.seat as usize] != i.hand {
                            return Err("the state does not match the infoset".into());
                        }
                        let sol = solver::solve(&mut s, table.as_mut(), *nodes);
                        if sol.capped {
                            // The referee counts these in the result file.
                            reply(&mut out, FromEngine::Info("capped".into()))?;
                        }
                        sol.best
                    }
                };
                reply(&mut out, FromEngine::Move(mv))?;
            }
            ToEngine::Quit => return Ok(()),
            ToEngine::Opening { .. }
            | ToEngine::Drew(_)
            | ToEngine::Played { .. }
            | ToEngine::RoundOver { .. }
            | ToEngine::GameOver { .. } => {}
        }
    }
    Ok(())
}

//! `dominofish engine --bot <name>`: a bot speaking the engine protocol
//! (docs/FORMATS.md, Engine protocol) on stdin and stdout. It decides from
//! the `infoset` line the referee sends before each `go`, with an RNG drawn
//! from the game's seed and the round, so a seed keyed by deal and seat
//! gives the same choices in both games of a duplicate pair. A salt mixes
//! into that seed, so two copies of one bot can be seeded independently
//! (the M2-08 null test).

use std::io::{BufRead, Write};

use dominofish_core::bots::Bot;
use dominofish_core::infoset::InfoSet;
use dominofish_core::protocol::{FromEngine, ToEngine, VERSION};
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::Mode;

pub fn run(bot: Bot, salt: Option<u64>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut mode = Mode::TwoVsTwo;
    let mut seed = 0u64;
    let mut rng = Rng::new(0);
    let mut info: Option<InfoSet> = None;
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
                reply(&mut out, FromEngine::Ready { version })?
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
            ToEngine::Round { round, .. } => rng = Rng::new(derive(seed, u64::from(round))),
            ToEngine::InfoSet(i) => info = Some(*i),
            ToEngine::Go => {
                let i = info.take().ok_or("go without an infoset")?;
                reply(&mut out, FromEngine::Move(bot.decide(&i, &mut rng)))?;
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

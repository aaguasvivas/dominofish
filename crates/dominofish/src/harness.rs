//! `dominofish match`: the referee of the match harness (M2-06). Every seat
//! is a separate engine process speaking the engine protocol, so partners
//! share nothing. The unit is a duplicate pair: one deal (in 1v1 with its
//! boneyard order) played twice with the sides swapping seats; full games
//! share the per-round deal seeds. Each seat's engine gets the seed
//! derive(deal seed, 100 + seat), keyed by deal and seat. Any illegal move,
//! crash, timeout, protocol error, or wrong header echo stops the match and
//! voids it (hard constraint 6).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use dominofish_core::infoset::{Action, InfoSet};
use dominofish_core::protocol::{FromEngine, ToEngine, VERSION};
use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, Mode, Move, Phase, State, team};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Round,
    Game,
}

/// One side of a match: its engine command, the commit its binary was built
/// from (the runs/bin/<commit>/ directory, else "-"), and the binary's hash.
#[derive(Clone, Debug)]
pub struct Side {
    pub command: Vec<String>,
    pub commit: String,
    pub hash: u64,
}

fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Side {
    /// A side from its command line. The binary may not come from target/:
    /// every match runs copies under runs/bin/<commit>/ (BUILD.md M2).
    pub fn new(command_line: &str) -> Result<Side, String> {
        let command: Vec<String> = command_line.split_whitespace().map(String::from).collect();
        let path = Path::new(command.first().ok_or("an empty engine command")?);
        if path.components().any(|c| c.as_os_str() == "target") {
            return Err(format!(
                "{}: matches run copies under runs/bin/<commit>/, never target/",
                path.display()
            ));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut parts = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        parts.pop();
        let commit = match parts.as_slice() {
            [.., runs, bin, commit] if runs == "runs" && bin == "bin" => commit.clone(),
            _ => "-".into(),
        };
        Ok(Side {
            command,
            commit,
            hash: fnv64(&bytes),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    Illegal,
    Crash,
    Timeout,
    Protocol,
    Header,
}

struct Engine {
    name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Engine {
    fn spawn(side: &Side, name: String) -> Result<Engine, String> {
        let mut child = Command::new(&side.command[0])
            .args(&side.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{name}: {e}"))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Engine {
            name,
            child,
            stdin,
            lines,
        })
    }

    fn send(&mut self, line: &str) -> Result<(), (Fault, String)> {
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| (Fault::Crash, format!("{}: write failed: {e}", self.name)))
    }

    /// The next reply that is not an info line.
    fn recv(&mut self, timeout: Duration) -> Result<FromEngine, (Fault, String)> {
        loop {
            let line = match self.lines.recv_timeout(timeout) {
                Ok(l) => l,
                Err(RecvTimeoutError::Timeout) => {
                    return Err((
                        Fault::Timeout,
                        format!("{}: no reply in {timeout:?}", self.name),
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err((Fault::Crash, format!("{}: exited", self.name)));
                }
            };
            match FromEngine::parse(&line) {
                Some(FromEngine::Info(_)) => continue,
                Some(msg) => return Ok(msg),
                None => {
                    return Err((
                        Fault::Protocol,
                        format!("{}: unreadable reply: {line}", self.name),
                    ));
                }
            }
        }
    }

    fn stop(mut self) {
        let _ = self.send("quit");
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Spec {
    pub mode: Mode,
    pub unit: Unit,
    pub target: u16,
    pub pairs: u64,
    /// Pair i plays deal seed `seed + i`.
    pub seed: u64,
    pub a: Side,
    pub b: Side,
    pub timeout: Duration,
}

/// One duplicate pair: the final scores of each game as [side A, side B].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairResult {
    pub pair: u64,
    pub deal_seed: u64,
    pub scores: [[u16; 2]; 2],
}

impl PairResult {
    /// Side A's points minus side B's, summed over the pair's two games.
    pub fn margin(&self) -> i32 {
        self.scores
            .iter()
            .map(|s| i32::from(s[0]) - i32::from(s[1]))
            .sum()
    }

    /// Games side A won (full games at the target).
    pub fn wins(&self, target: u16) -> u32 {
        self.scores.iter().filter(|s| s[0] >= target).count() as u32
    }
}

pub struct Report {
    pub header: String,
    pub pairs: Vec<PairResult>,
    pub moves: u64,
    pub fault: Option<(Fault, String)>,
}

impl Spec {
    /// The match header each engine must echo before the first deal.
    pub fn header(&self) -> String {
        let side = |name: &str, s: &Side| {
            format!(
                "{name}.cmd={} {name}.commit={} {name}.bench=- {name}.hash={:016x}",
                s.command.join("+"),
                s.commit,
                s.hash
            )
        };
        format!(
            "{} {} mode={} unit={} target={} pairs={} seeds={}..{} budgets=- threads=1 tables=-",
            side("a", &self.a),
            side("b", &self.b),
            if self.mode == Mode::OneVsOne {
                "1v1"
            } else {
                "2v2"
            },
            if self.unit == Unit::Round {
                "round"
            } else {
                "game"
            },
            self.target,
            self.pairs,
            self.seed,
            self.seed + self.pairs.saturating_sub(1)
        )
    }
}

/// Plays the match; stops at the first fault.
pub fn run(spec: &Spec) -> Report {
    let header = spec.header();
    let mut report = Report {
        header: header.clone(),
        pairs: Vec::new(),
        moves: 0,
        fault: None,
    };
    let per_side = (spec.mode.seats() / 2) as usize;
    let mut engines: Vec<Engine> = Vec::new();
    for (side, name) in [(&spec.a, "a"), (&spec.b, "b")] {
        for k in 0..per_side {
            match Engine::spawn(side, format!("{name}{k}")) {
                Ok(e) => engines.push(e),
                Err(e) => {
                    report.fault = Some((Fault::Crash, e));
                    engines.into_iter().for_each(Engine::stop);
                    return report;
                }
            }
        }
    }
    let result = handshake(&mut engines, &header, spec.timeout).and_then(|_| {
        for i in 0..spec.pairs {
            let deal_seed = spec.seed + i;
            let mut scores = [[0u16; 2]; 2];
            for (g, out) in scores.iter_mut().enumerate() {
                // engine_at[seat]: side A's engines take team g's seats.
                let engine_at: Vec<usize> = (0..spec.mode.seats())
                    .map(|seat| {
                        let a_side = team(seat) == g;
                        let k = (seat / 2) as usize;
                        if a_side { k } else { per_side + k }
                    })
                    .collect();
                let final_scores =
                    play_game(spec, &mut engines, &engine_at, deal_seed, &mut report.moves)?;
                *out = [final_scores[g], final_scores[1 - g]];
            }
            report.pairs.push(PairResult {
                pair: i,
                deal_seed,
                scores,
            });
        }
        Ok(())
    });
    if let Err(f) = result {
        report.fault = Some(f);
    }
    engines.into_iter().for_each(Engine::stop);
    report
}

fn handshake(
    engines: &mut [Engine],
    header: &str,
    timeout: Duration,
) -> Result<(), (Fault, String)> {
    for e in engines.iter_mut() {
        e.send(&ToEngine::Hello { version: VERSION }.to_line(Mode::TwoVsTwo))?;
        match e.recv(timeout)? {
            FromEngine::Ready { version } if version == VERSION => {}
            other => {
                return Err((
                    Fault::Protocol,
                    format!("{}: expected ready, got {other:?}", e.name),
                ));
            }
        }
        e.send(&ToEngine::Header(header.into()).to_line(Mode::TwoVsTwo))?;
        match e.recv(timeout)? {
            FromEngine::Header(h) if h == header => {}
            other => {
                return Err((
                    Fault::Header,
                    format!("{}: header echo differs: {other:?}", e.name),
                ));
            }
        }
    }
    Ok(())
}

/// One game (or its first round, for the round unit); returns the final
/// scores by team.
fn play_game(
    spec: &Spec,
    engines: &mut [Engine],
    engine_at: &[usize],
    deal_seed: u64,
    moves: &mut u64,
) -> Result<[u16; 2], (Fault, String)> {
    let mode = spec.mode;
    let deal_for = |r: u64| Deal::shuffled(&mut Rng::new(derive(deal_seed, r)));
    for seat in 0..mode.seats() {
        let msg = ToEngine::Game {
            mode,
            target: spec.target,
            seat,
            seed: derive(deal_seed, 100 + u64::from(seat)),
        };
        engines[engine_at[seat as usize]].send(&msg.to_line(mode))?;
    }
    let mut record = GameRecord {
        mode,
        target: spec.target,
        rounds: vec![RoundRecord {
            deal: deal_for(0),
            moves: vec![],
        }],
    };
    let (mut state, opening) = State::first_round(mode, spec.target, &record.rounds[0].deal);
    let mut opening = Some(opening);
    loop {
        let r = record.rounds.len() - 1;
        // Round start: each engine's own dealt hand, then round 1's opening.
        for seat in 0..mode.seats() {
            let msg = ToEngine::Round {
                round: r as u32 + 1,
                scores: state.scores,
                leader: state.starter,
                hand: record.rounds[r].deal.hand(seat).to_vec(),
            };
            engines[engine_at[seat as usize]].send(&msg.to_line(mode))?;
        }
        if let Some(tile) = opening.take() {
            let msg = ToEngine::Opening {
                seat: state.starter,
                tile,
            }
            .to_line(mode);
            for seat in 0..mode.seats() {
                engines[engine_at[seat as usize]].send(&msg)?;
            }
        }
        while state.phase == Phase::Playing {
            let mover = state.turn;
            let e = engine_at[mover as usize];
            let ply = record.rounds[r].moves.len();
            let info = InfoSet::new(&record, r, ply, mover).expect("the record so far is valid");
            engines[e].send(&ToEngine::InfoSet(Box::new(info)).to_line(mode))?;
            engines[e].send(&ToEngine::Go.to_line(mode))?;
            let mv = match engines[e].recv(spec.timeout)? {
                FromEngine::Move(mv) => mv,
                other => {
                    return Err((
                        Fault::Protocol,
                        format!("{}: expected a move, got {other:?}", engines[e].name),
                    ));
                }
            };
            if !state.is_legal(mv) {
                return Err((
                    Fault::Illegal,
                    format!(
                        "{}: illegal move {mv:?} at deal seed {deal_seed} round {} ply {ply}",
                        engines[e].name,
                        r + 1
                    ),
                ));
            }
            let pile = state.boneyard().to_vec();
            state.play(mv);
            record.rounds[r].moves.push(mv);
            *moves += 1;
            let action = match mv {
                Move::Play { tile, end } => Action::Play { tile, end },
                Move::Draw => {
                    let drawn: Vec<_> = pile[state.boneyard().len()..]
                        .iter()
                        .rev()
                        .copied()
                        .collect();
                    engines[e].send(&ToEngine::Drew(drawn.clone()).to_line(mode))?;
                    Action::Draw {
                        count: drawn.len() as u8,
                    }
                }
                Move::Pass => Action::Pass,
            };
            let msg = ToEngine::Played {
                seat: mover,
                action,
            }
            .to_line(mode);
            for seat in 0..mode.seats() {
                engines[engine_at[seat as usize]].send(&msg)?;
            }
        }
        let done = state.phase == Phase::Finished || spec.unit == Unit::Round;
        let msg = if done {
            ToEngine::GameOver {
                scores: state.scores,
            }
        } else {
            ToEngine::RoundOver {
                scores: state.scores,
            }
        };
        for seat in 0..mode.seats() {
            engines[engine_at[seat as usize]].send(&msg.to_line(mode))?;
        }
        if done {
            return Ok(state.scores);
        }
        let deal = deal_for(record.rounds.len() as u64);
        state = state.next_round(&deal);
        record.rounds.push(RoundRecord {
            deal,
            moves: vec![],
        });
    }
}

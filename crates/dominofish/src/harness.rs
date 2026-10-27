//! `dominofish match`: the referee of the match harness (M2-06). Every seat
//! is a separate engine process speaking the engine protocol, so partners
//! share nothing. The unit is a duplicate pair: one deal (in 1v1 with its
//! boneyard order) played twice with the sides swapping seats; full games
//! share the per-round deal seeds. Each seat's engine gets the seed
//! derive(derive(engine seed, pair), 100 + seat), keyed by pair and seat
//! from a base apart from the deal seeds, and the header sent to engines
//! holds no deal seed, so no engine message is a function of a deal. Any
//! illegal move, crash, timeout, protocol error, line out of turn, or wrong
//! header echo stops the match and voids it (hard constraint 6).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dominofish_core::aivat::{self, Valuer};
use dominofish_core::bots::Bot;
use dominofish_core::infoset::{Action, InfoSet};
use dominofish_core::protocol::{FromEngine, ToEngine, VERSION, View};
use dominofish_core::record::{GameRecord, RoundRecord};
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, Mode, Move, MoveList, Phase, State, team};

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
    /// The baseline bot the command runs (`engine --bot <name>`), whose
    /// policy the AIVAT action terms use; None for any other engine.
    pub bot: Option<Bot>,
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
        let bot = command
            .windows(2)
            .find(|w| w[0] == "--bot")
            .and_then(|w| Bot::from_name(&w[1]));
        Ok(Side {
            command,
            commit,
            hash: fnv64(&bytes),
            bot,
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
    /// A PIMC engine drew a world that disagrees with its InfoSet, or could
    /// draw none (`info inconsistent`, `info fallback`; docs/PLAN.md D17).
    Sampler,
}

struct Engine {
    name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    /// The end of what the engine wrote to stderr, for a crash report.
    stderr: Arc<Mutex<String>>,
    /// What the engine sees, from its ready line: only an omniscient engine
    /// (the cheating upper bound) is also sent the full state.
    view: View,
    /// `info capped` lines: searches that reached their node cap.
    capped: u64,
    /// Time from each `go` to its move: total and slowest, in nanoseconds,
    /// and the number of moves.
    think: Think,
    /// The side's baseline bot, if any (`Side::bot`).
    bot: Option<Bot>,
}

/// Reply times to `go`: a record, not a budget (budgets are nodes or
/// worlds, BUILD.md hard constraint 7).
#[derive(Clone, Copy, Default)]
pub struct Think {
    pub total_ns: u64,
    pub max_ns: u64,
    pub moves: u64,
}

impl Think {
    fn add(&mut self, other: Think) {
        self.total_ns += other.total_ns;
        self.max_ns = self.max_ns.max(other.max_ns);
        self.moves += other.moves;
    }
}

/// Bytes of an engine's stderr kept for a crash report.
const STDERR_TAIL: usize = 2000;

impl Engine {
    fn spawn(side: &Side, name: String) -> Result<Engine, String> {
        let mut child = Command::new(&side.command[0])
            .args(&side.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{name}: {e}"))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let err_pipe = child.stderr.take().expect("piped stderr");
        let (tx, lines) = channel();
        // A line that is not UTF-8 arrives lossily converted, so it fails to
        // parse as a protocol fault instead of ending the stream.
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']).to_string();
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let tail = Arc::clone(&stderr);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(err_pipe);
            let mut buf = Vec::new();
            while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                let mut t = tail.lock().expect("stderr lock");
                t.push_str(&String::from_utf8_lossy(&buf));
                if t.len() > STDERR_TAIL {
                    let cut = t.len() - STDERR_TAIL;
                    let cut = (cut..t.len())
                        .find(|&i| t.is_char_boundary(i))
                        .unwrap_or(t.len());
                    t.drain(..cut);
                }
                buf.clear();
            }
        });
        Ok(Engine {
            name,
            child,
            stdin,
            lines,
            stderr,
            view: View::InfoSet,
            capped: 0,
            think: Think::default(),
            bot: side.bot,
        })
    }

    fn send(&mut self, line: &str) -> Result<(), (Fault, String)> {
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| (Fault::Crash, format!("{}: write failed: {e}", self.name)))
    }

    /// The next reply that is not an info line, within one deadline for the
    /// whole reply: info lines do not extend it.
    fn recv(&mut self, timeout: Duration) -> Result<FromEngine, (Fault, String)> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(l) => l,
                Err(RecvTimeoutError::Timeout) => {
                    return Err((
                        Fault::Timeout,
                        format!("{}: no reply in {timeout:?}", self.name),
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err((Fault::Crash, self.exited()));
                }
            };
            match FromEngine::parse(&line) {
                Some(FromEngine::Info(text)) => {
                    if text == "inconsistent" || text == "fallback" {
                        return Err((
                            Fault::Sampler,
                            format!("{}: the sampler reported {text}", self.name),
                        ));
                    }
                    self.capped += u64::from(text == "capped");
                    continue;
                }
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

    /// A crash report: the engine exited, with the end of its stderr.
    fn exited(&self) -> String {
        // The stderr reader may still be draining the pipe.
        std::thread::sleep(Duration::from_millis(50));
        let tail = self.stderr.lock().expect("stderr lock");
        let tail = tail.trim();
        if tail.is_empty() {
            format!("{}: exited", self.name)
        } else {
            format!(
                "{}: exited; stderr: {}",
                self.name,
                tail.replace('\n', " | ")
            )
        }
    }

    /// Fails on any line other than info that the engine sent unasked (a
    /// reply out of turn), so a stray line can never pass for a later move.
    fn expect_quiet(&mut self) -> Result<(), (Fault, String)> {
        while let Ok(line) = self.lines.try_recv() {
            if !matches!(FromEngine::parse(&line), Some(FromEngine::Info(_))) {
                return Err((
                    Fault::Protocol,
                    format!("{}: sent a line out of turn: {line}", self.name),
                ));
            }
        }
        Ok(())
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
    /// The base of the engines' seeds, apart from the deal seeds: seat s of
    /// pair i gets derive(derive(engine_seed, i), 100 + s), keyed by pair
    /// and seat, so no engine message is a function of a deal.
    pub engine_seed: u64,
    /// AIVAT with this many sampled deals and boneyard orders per term
    /// (docs/PLAN.md D19); None leaves the result uncorrected.
    pub aivat: Option<u32>,
}

/// A finished game: its final scores, its rounds, and team 0's AIVAT
/// terms (deal, draw, action).
type GameEnd = ([u16; 2], u32, [f64; 3]);

/// The node cap of each AIVAT value (D19).
pub const AIVAT_NODES: u64 = 2_000_000;

/// A pair's AIVAT terms, each summed over both games from side A's view.
/// The corrected result subtracts them with cross-fitted coefficients
/// (`stats::crossfit`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairAivat {
    pub deal: f64,
    pub draw: f64,
    pub action: f64,
}

/// One duplicate pair: the final scores of each game as [side A, side B],
/// and the rounds each game lasted.
#[derive(Clone, Debug, PartialEq)]
pub struct PairResult {
    pub pair: u64,
    pub deal_seed: u64,
    pub scores: [[u16; 2]; 2],
    pub rounds: [u32; 2],
    pub aivat: Option<PairAivat>,
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

    /// Side A's margin per round over the pair.
    pub fn margin_per_round(&self) -> f64 {
        f64::from(self.margin()) / f64::from(self.rounds[0] + self.rounds[1])
    }

    /// The pair's margin per round and its AIVAT terms per round (deal,
    /// draw, action), the row `stats::crossfit` takes.
    pub fn aivat_row(&self) -> Option<(f64, [f64; 3])> {
        let rounds = f64::from(self.rounds[0] + self.rounds[1]);
        self.aivat.map(|a| {
            (
                self.margin_per_round(),
                [a.deal / rounds, a.draw / rounds, a.action / rounds],
            )
        })
    }
}

pub struct Report {
    pub header: String,
    pub pairs: Vec<PairResult>,
    pub moves: u64,
    pub fault: Option<(Fault, String)>,
    /// Each side's searches that reached their node cap (`info capped`).
    pub capped: [u64; 2],
    /// Each side's reply times to `go`.
    pub think: [Think; 2],
    /// AIVAT values computed, those that reached the node cap, and whether
    /// draw terms were on (off when an engine sees hidden state).
    pub aivat_values: u64,
    pub aivat_capped: u64,
    pub aivat_draws: bool,
}

/// An engine's own settings, from its ready line.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Settings {
    budgets: String,
    threads: u32,
    tables: String,
    view: View,
}

/// The Bench of an engine binary that searches: the last line of
/// `<binary> bench`, run once before the match. - when the program is not
/// a dominofish binary (a wrapper script would take "bench" as its own
/// argument) or prints no Bench line.
fn bench_of(side: &Side) -> String {
    if Path::new(&side.command[0]).file_name() != Some("dominofish".as_ref()) {
        return "-".into();
    }
    Command::new(&side.command[0])
        .arg("bench")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .last()
                .and_then(|l| l.strip_prefix("Bench: "))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "-".into())
}

impl Spec {
    /// The match header each engine must echo before the first deal: each
    /// side's command, commit, Bench, binary hash, and the settings its
    /// engines reported; then mode, unit, target, and pairs. It holds no
    /// seed: the deal seeds go only to the results file.
    fn header(&self, settings: &[Settings; 2]) -> String {
        // A side that searches (one that reports budgets: the omniscient
        // bot, PIMC) gets its binary's Bench.
        let side = |name: &str, s: &Side, set: &Settings| {
            let bench = if set.budgets != "-" {
                bench_of(s)
            } else {
                "-".into()
            };
            format!(
                "{name}.cmd={} {name}.commit={} {name}.bench={bench} {name}.hash={:016x} {name}.budgets={} {name}.threads={} {name}.tables={} {name}.view={}",
                s.command.join("+"),
                s.commit,
                s.hash,
                set.budgets,
                set.threads,
                set.tables,
                set.view.name()
            )
        };
        format!(
            "{} {} mode={} unit={} target={} pairs={}",
            side("a", &self.a, &settings[0]),
            side("b", &self.b, &settings[1]),
            self.mode.name(),
            if self.unit == Unit::Round {
                "round"
            } else {
                "game"
            },
            self.target,
            self.pairs
        )
    }
}

/// Plays the match; stops at the first fault, or after a pair when `stop`
/// says so (an SPRT decision).
pub fn run(spec: &Spec, stop: &mut dyn FnMut(&PairResult) -> bool) -> Report {
    let mut report = Report {
        header: String::new(),
        pairs: Vec::new(),
        moves: 0,
        fault: None,
        capped: [0, 0],
        think: [Think::default(); 2],
        aivat_values: 0,
        aivat_capped: 0,
        aivat_draws: false,
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
    let mut valuer = spec.aivat.map(|_| Valuer::new(16, AIVAT_NODES));
    let result = handshake(spec, &mut engines, per_side, &mut report.header).and_then(|_| {
        // A draw term needs a boneyard order no engine sees.
        let draws = engines.iter().all(|e| e.view == View::InfoSet);
        report.aivat_draws = spec.aivat.is_some() && draws;
        for i in 0..spec.pairs {
            let deal_seed = spec.seed + i;
            let mut scores = [[0u16; 2]; 2];
            let mut rounds = [0u32; 2];
            // Side A's terms over both games: deal, draw, action.
            let mut terms = [0.0f64; 3];
            for g in 0..2 {
                // engine_at[seat]: side A's engines take team g's seats.
                let engine_at: Vec<usize> = (0..spec.mode.seats())
                    .map(|seat| {
                        let a_side = team(seat) == g;
                        let k = (seat / 2) as usize;
                        if a_side { k } else { per_side + k }
                    })
                    .collect();
                let (final_scores, played, team0_terms) = play_game(
                    spec,
                    &mut engines,
                    &engine_at,
                    i,
                    deal_seed,
                    &mut report.moves,
                    valuer.as_mut().map(|v| (v, draws)),
                )?;
                scores[g] = [final_scores[g], final_scores[1 - g]];
                rounds[g] = played;
                // Side A holds team g's seats: team 0's terms count for A
                // in game 0 and against A in game 1.
                let sign = if g == 0 { 1.0 } else { -1.0 };
                for (t, x) in terms.iter_mut().zip(team0_terms) {
                    *t += sign * x;
                }
            }
            let pair = PairResult {
                pair: i,
                deal_seed,
                scores,
                rounds,
                aivat: spec.aivat.map(|_| PairAivat {
                    deal: terms[0],
                    draw: terms[1],
                    action: terms[2],
                }),
            };
            let done = stop(&pair);
            report.pairs.push(pair);
            if done {
                break;
            }
        }
        Ok(())
    });
    if let Err(f) = result {
        report.fault = Some(f);
    }
    for (k, e) in engines.iter().enumerate() {
        report.capped[k / per_side] += e.capped;
        report.think[k / per_side].add(e.think);
    }
    if let Some(v) = &valuer {
        report.aivat_values = v.values;
        report.aivat_capped = v.capped;
    }
    engines.into_iter().for_each(Engine::stop);
    report
}

/// Version and settings from every engine, then the header, which each
/// engine must echo. The engines of one side must report the same settings.
fn handshake(
    spec: &Spec,
    engines: &mut [Engine],
    per_side: usize,
    header: &mut String,
) -> Result<(), (Fault, String)> {
    let mut reported: Vec<Settings> = Vec::new();
    for e in engines.iter_mut() {
        e.send(&ToEngine::Hello { version: VERSION }.to_line(Mode::TwoVsTwo))?;
        match e.recv(spec.timeout)? {
            FromEngine::Ready {
                version,
                budgets,
                threads,
                tables,
                view,
            } if version == VERSION => {
                e.view = view;
                reported.push(Settings {
                    budgets,
                    threads,
                    tables,
                    view,
                });
            }
            other => {
                return Err((
                    Fault::Protocol,
                    format!("{}: expected ready, got {other:?}", e.name),
                ));
            }
        }
    }
    for (k, set) in reported.iter().enumerate() {
        if *set != reported[k / per_side * per_side] {
            return Err((
                Fault::Protocol,
                format!(
                    "{}: settings differ from its side's: {set:?}",
                    engines[k].name
                ),
            ));
        }
    }
    *header = spec.header(&[reported[0].clone(), reported[per_side].clone()]);
    for e in engines.iter_mut() {
        e.send(&ToEngine::Header(header.clone()).to_line(Mode::TwoVsTwo))?;
        match e.recv(spec.timeout)? {
            FromEngine::Header(h) if h == *header => {}
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
/// scores by team and the rounds played.
fn play_game(
    spec: &Spec,
    engines: &mut [Engine],
    engine_at: &[usize],
    pair: u64,
    deal_seed: u64,
    moves: &mut u64,
    mut aivat: Option<(&mut Valuer, bool)>,
) -> Result<GameEnd, (Fault, String)> {
    let mode = spec.mode;
    let deal_for = |r: u64| Deal::shuffled(&mut Rng::new(derive(deal_seed, r)));
    // AIVAT (D19): team 0's deal, draw, and action terms, the sampled
    // deals and boneyard orders keyed by the deal seed, apart from it.
    let mut terms = [0.0f64; 3];
    let j = spec.aivat.unwrap_or(0);
    let mut reshuffle = Rng::new(derive(deal_seed, 1_000_000));
    for seat in 0..mode.seats() {
        let msg = ToEngine::Game {
            mode,
            target: spec.target,
            seat,
            seed: derive(derive(spec.engine_seed, pair), 100 + u64::from(seat)),
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
            // A baseline bot's exact policy, for its action term.
            let policy = match (&aivat, engines[e].bot) {
                (Some(_), Some(bot)) => {
                    let mut p = MoveList::new();
                    bot.policy(&info, &mut p);
                    Some(p)
                }
                _ => None,
            };
            let before = state;
            engines[e].expect_quiet()?;
            engines[e].send(&ToEngine::InfoSet(Box::new(info)).to_line(mode))?;
            if engines[e].view == View::Omniscient {
                engines[e].send(&ToEngine::State(Box::new(state)).to_line(mode))?;
            }
            engines[e].send(&ToEngine::Go.to_line(mode))?;
            let asked = Instant::now();
            let reply = engines[e].recv(spec.timeout)?;
            let ns = asked.elapsed().as_nanos() as u64;
            engines[e].think.add(Think {
                total_ns: ns,
                max_ns: ns,
                moves: 1,
            });
            let mv = match reply {
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
            if let Some((v, draws)) = aivat.as_mut() {
                if let Some(p) = &policy {
                    terms[2] += aivat::action_term(v, &before, mv, p.as_slice());
                }
                if mv == Move::Draw && *draws {
                    terms[1] += aivat::draw_term(v, &before, &mut reshuffle, j);
                }
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
            return Ok((state.scores, record.rounds.len() as u32, terms));
        }
        let next = record.rounds.len() as u64;
        let deal = deal_for(next);
        if let Some((v, _)) = aivat.as_mut() {
            // The same sampled deals in both games of the pair.
            let others: Vec<Deal> = (0..u64::from(j))
                .map(|k| Deal::shuffled(&mut Rng::new(derive(derive(deal_seed, next), 500 + k))))
                .collect();
            terms[0] += aivat::deal_term(v, &state, &deal, &others);
        }
        state = state.next_round(&deal);
        record.rounds.push(RoundRecord {
            deal,
            moves: vec![],
        });
    }
}

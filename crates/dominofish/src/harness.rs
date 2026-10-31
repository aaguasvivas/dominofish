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

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
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
    /// The baseline bot the command runs (`engine --bot <name>`); None for
    /// any other engine. The AIVAT action terms use its policy only when the
    /// side runs the referee's own binary (`run`).
    pub bot: Option<Bot>,
}

fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The hash of the running binary, the referee's, as `Side::hash` is a
/// side's.
fn own_hash() -> Option<u64> {
    let path = std::env::current_exe().ok()?;
    std::fs::read(path).ok().map(|bytes| fnv64(&bytes))
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
    /// The baseline bot whose policy the AIVAT action terms use: the side's
    /// (`Side::bot`) when the side runs the referee's own binary, else None.
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
    fn spawn(side: &Side, bot: Option<Bot>, name: String) -> Result<Engine, String> {
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
            bot,
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

    /// After the engine's last pair: a line other than info that it has
    /// sent, or its exit, is a fault, as it would be at its next pair. A
    /// line still on its way is not seen.
    fn check_quiet(&mut self) -> Result<(), (Fault, String)> {
        self.expect_quiet()?;
        match self.child.try_wait() {
            Ok(Some(_)) => Err((Fault::Crash, self.exited())),
            _ => Ok(()),
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
    /// The base of the engines' seeds, apart from the deal seeds: seat s of
    /// pair i gets derive(derive(engine_seed, i), 100 + s), keyed by pair
    /// and seat, so no engine message is a function of a deal.
    pub engine_seed: u64,
    /// AIVAT with this many sampled deals and boneyard orders per term
    /// (docs/PLAN.md D19); None leaves the result uncorrected.
    pub aivat: Option<u32>,
    /// Pairs played at once, each by its own set of engines. A pair's
    /// result depends only on its index (its deal and engine seeds), and the
    /// referee takes the pairs in index order, so a match without a fault
    /// gives the same report at any count, reply times aside (`schedule`).
    pub threads: usize,
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

    /// The pair's margin per round and its AIVAT term totals (deal, draw,
    /// action), the row `stats::crossfit` takes. The terms stay totals: in
    /// full games the number of rounds depends on the luck, so a term
    /// divided by it loses its zero mean (docs/PLAN.md D19 and Q12).
    pub fn aivat_row(&self) -> Option<(f64, [f64; 3])> {
        self.aivat
            .map(|a| (self.margin_per_round(), [a.deal, a.draw, a.action]))
    }
}

#[derive(Default)]
pub struct Report {
    pub header: String,
    pub pairs: Vec<PairResult>,
    pub moves: u64,
    pub fault: Option<(Fault, String)>,
    /// Each side's searches that reached their node cap (`info capped`).
    pub capped: [u64; 2],
    /// Each side's reply times to `go`.
    pub think: [Think; 2],
    /// AIVAT values computed, those that reached the node cap, whether
    /// draw terms were on (off when an engine sees hidden state), and which
    /// sides' moves had action terms [A, B].
    pub aivat_values: u64,
    pub aivat_capped: u64,
    pub aivat_draws: bool,
    pub aivat_actions: [bool; 2],
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

/// Plays the match, `spec.threads` pairs at once, and stops after a pair
/// when `stop` says so (an SPRT decision) or at a fault: then it reports the
/// fault of the lowest pair index once every pair in flight has ended
/// (`schedule`). After the last pair, a line out of turn or an exit of any
/// engine is also a fault.
pub fn run(spec: &Spec, stop: &mut dyn FnMut(&PairResult) -> bool) -> Report {
    let mut report = Report::default();
    let per_side = (spec.mode.seats() / 2) as usize;
    // An action term weighs a bot's move with the referee's own
    // Bot::policy, so only a side that runs this same binary gets one: a
    // bot from another commit may break its ties another way.
    let own = spec.aivat.and_then(|_| own_hash());
    // One set of engines per thread: the first set's are a0, b0, and so on,
    // set t's a0.t, b0.t.
    let mut sets: Vec<Vec<Engine>> = Vec::new();
    'spawn: for t in 0..spec.threads {
        let mut engines = Vec::new();
        for (s, (side, name)) in [(&spec.a, "a"), (&spec.b, "b")].into_iter().enumerate() {
            let bot = side.bot.filter(|_| own == Some(side.hash));
            report.aivat_actions[s] = bot.is_some();
            for k in 0..per_side {
                let label = if t == 0 {
                    format!("{name}{k}")
                } else {
                    format!("{name}{k}.{t}")
                };
                match Engine::spawn(side, bot, label) {
                    Ok(e) => engines.push(e),
                    Err(e) => {
                        report.fault = Some((Fault::Crash, e));
                        sets.push(engines);
                        break 'spawn;
                    }
                }
            }
        }
        sets.push(engines);
    }
    if report.fault.is_none() {
        let mut valuers: Vec<Option<Valuer>> = sets
            .iter()
            .map(|_| spec.aivat.map(|_| Valuer::new(16, AIVAT_NODES)))
            .collect();
        let result = handshake(spec, &mut sets, per_side, &mut report.header).and_then(|_| {
            // A draw term needs a boneyard order no engine sees.
            let draws = sets[0].iter().all(|e| e.view == View::InfoSet);
            report.aivat_draws = spec.aivat.is_some() && draws;
            play_pairs(
                spec,
                &mut sets,
                &mut valuers,
                per_side,
                draws,
                stop,
                &mut report,
            )?;
            // A line out of turn or an exit after a set's last pair is a
            // fault, as it would be at that set's next pair.
            sets.iter_mut().flatten().try_for_each(Engine::check_quiet)
        });
        if let Err(f) = result {
            report.fault = Some(f);
        }
    }
    sets.into_iter().flatten().for_each(Engine::stop);
    report
}

/// What one pair cost: its moves, each side's capped searches and reply
/// times, and the AIVAT values computed and capped.
#[derive(Clone, Copy, Default)]
struct PairStats {
    moves: u64,
    capped: [u64; 2],
    think: [Think; 2],
    aivat_values: u64,
    aivat_capped: u64,
}

impl Report {
    fn add(&mut self, s: &PairStats) {
        self.moves += s.moves;
        for k in 0..2 {
            self.capped[k] += s.capped[k];
            self.think[k].add(s.think[k]);
        }
        self.aivat_values += s.aivat_values;
        self.aivat_capped += s.aivat_capped;
    }
}

/// One pair from a worker: the pair or its fault, and what it cost.
type Played = (Result<PairResult, (Fault, String)>, PairStats);

/// The pairs handed out, the pairs the referee has taken in index order,
/// and whether to hand out no more.
struct Gate {
    next: u64,
    taken: u64,
    halt: bool,
}

/// The gate the workers and the referee share. A thread that panics while
/// it holds the lock leaves the gate as it was, so the others go on and the
/// panic reaches the caller when the threads join.
struct Shared {
    gate: Mutex<Gate>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Gate> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hands out no more pairs; a worker still finishes the one it plays.
    fn halt(&self) {
        self.lock().halt = true;
        self.wake.notify_all();
    }

    fn set_taken(&self, taken: u64) {
        self.lock().taken = taken;
        self.wake.notify_all();
    }

    /// The next pair for a worker, which waits while `window` pairs past
    /// the last one taken are out; None once halted or every pair is out.
    fn next(&self, pairs: u64, window: u64) -> Option<u64> {
        let mut g = self.lock();
        while !g.halt && g.next < pairs && g.next >= g.taken + window {
            g = self.wake.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
        if g.halt || g.next >= pairs {
            return None;
        }
        g.next += 1;
        Some(g.next - 1)
    }
}

/// Halts the gate when its thread unwinds from a panic, so no thread waits
/// for a pair that never comes, and the panic reaches the caller when the
/// threads join.
struct HaltOnPanic<'a>(&'a Shared);

impl Drop for HaltOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.halt();
        }
    }
}

/// Plays the pairs, each set of engines with its own AIVAT valuer on its
/// own thread (`schedule`).
fn play_pairs(
    spec: &Spec,
    sets: &mut [Vec<Engine>],
    valuers: &mut [Option<Valuer>],
    per_side: usize,
    draws: bool,
    stop: &mut dyn FnMut(&PairResult) -> bool,
    report: &mut Report,
) -> Result<(), (Fault, String)> {
    let mut workers: Vec<_> = sets.iter_mut().zip(valuers.iter_mut()).collect();
    let play = |w: &mut (&mut Vec<Engine>, &mut Option<Valuer>), i: u64| {
        measured(spec, w.0, per_side, i, w.1.as_mut(), draws)
    };
    schedule(spec.pairs, &mut workers, &play, stop, report)
}

/// Hands out the pair indices 0, 1, 2, and so on to `workers`, each on its
/// own thread, and takes the results in index order. A worker starts pair
/// i only while i is below the pairs taken plus the number of workers, so a
/// single worker plays the pairs strictly in turn. `stop` sees the pairs in
/// index order, and a pair started past its stop is discarded. A fault in
/// any pair played, one past a stop included, voids the match: the referee
/// takes every pair below the fault up to a stop, waits for every pair in
/// flight, and returns the fault of the lowest index, with that pair's
/// cost. So when a pair's result and faults depend only on its index, the
/// report is the same at any number of workers, faults included, apart from
/// the engine name in a fault and a fault in a pair past a stop, which a
/// single worker never plays. A fault an engine commits between pairs (a
/// line out of turn, an exit) shows at that engine's next pair, so its pair
/// index can change with the number of workers.
fn schedule<W: Send>(
    pairs: u64,
    workers: &mut [W],
    play: &(dyn Fn(&mut W, u64) -> Played + Sync),
    stop: &mut dyn FnMut(&PairResult) -> bool,
    report: &mut Report,
) -> Result<(), (Fault, String)> {
    let window = workers.len() as u64;
    let shared = Shared {
        gate: Mutex::new(Gate {
            next: 0,
            taken: 0,
            halt: false,
        }),
        wake: Condvar::new(),
    };
    std::thread::scope(|scope| {
        // Before the spawns, so that a failed spawn also halts the workers
        // already started.
        let _halt = HaltOnPanic(&shared);
        let (tx, rx) = channel();
        for w in workers.iter_mut() {
            let (tx, shared) = (tx.clone(), &shared);
            scope.spawn(move || {
                let _halt = HaltOnPanic(shared);
                while let Some(i) = shared.next(pairs, window) {
                    let (result, stats) = play(w, i);
                    let failed = result.is_err();
                    if tx.send((i, result, stats)).is_err() || failed {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut pending: BTreeMap<u64, (PairResult, PairStats)> = BTreeMap::new();
        // The fault of the lowest pair index, with that pair's cost.
        let mut fault: Option<(u64, (Fault, String), PairStats)> = None;
        let (mut taken, mut stopped) = (0, false);
        // Every result until the last worker ends, so that a fault in a
        // pair in flight at a stop or at another fault still counts.
        for (i, result, stats) in rx.iter() {
            match result {
                Ok(pair) => {
                    pending.insert(i, (pair, stats));
                }
                Err(f) => {
                    if fault.as_ref().is_none_or(|(j, ..)| i < *j) {
                        fault = Some((i, f, stats));
                    }
                }
            }
            let limit = fault.as_ref().map_or(pairs, |(j, ..)| *j);
            while !stopped && taken < limit {
                let Some((pair, stats)) = pending.remove(&taken) else {
                    break;
                };
                report.add(&stats);
                stopped = stop(&pair);
                report.pairs.push(pair);
                taken += 1;
            }
            if stopped || fault.is_some() || taken == pairs {
                shared.halt();
            } else {
                shared.set_taken(taken);
            }
        }
        match fault {
            Some((_, f, stats)) => {
                report.add(&stats);
                Err(f)
            }
            None => Ok(()),
        }
    })
}

/// Pair i with one set of engines, and what it cost, also when it fails;
/// a fault names its pair.
fn measured(
    spec: &Spec,
    engines: &mut [Engine],
    per_side: usize,
    i: u64,
    mut valuer: Option<&mut Valuer>,
    draws: bool,
) -> Played {
    let capped: Vec<u64> = engines.iter().map(|e| e.capped).collect();
    let values = valuer.as_ref().map_or((0, 0), |v| (v.values, v.capped));
    for e in engines.iter_mut() {
        e.think = Think::default();
    }
    let mut stats = PairStats::default();
    let result = play_pair(
        spec,
        engines,
        per_side,
        i,
        valuer.as_deref_mut(),
        draws,
        &mut stats.moves,
    )
    .map_err(|(kind, detail)| (kind, format!("pair {i}: {detail}")));
    for (k, e) in engines.iter().enumerate() {
        stats.capped[k / per_side] += e.capped - capped[k];
        stats.think[k / per_side].add(e.think);
    }
    if let Some(v) = valuer {
        stats.aivat_values = v.values - values.0;
        stats.aivat_capped = v.capped - values.1;
    }
    (result, stats)
}

/// Pair i: its deal played twice, side A on team 0's seats first, then on
/// team 1's.
fn play_pair(
    spec: &Spec,
    engines: &mut [Engine],
    per_side: usize,
    i: u64,
    mut valuer: Option<&mut Valuer>,
    draws: bool,
    moves: &mut u64,
) -> Result<PairResult, (Fault, String)> {
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
            engines,
            &engine_at,
            i,
            deal_seed,
            moves,
            valuer.as_deref_mut().map(|v| (v, draws)),
        )?;
        scores[g] = [final_scores[g], final_scores[1 - g]];
        rounds[g] = played;
        // Side A holds team g's seats: team 0's terms count for A in game 0
        // and against A in game 1.
        let sign = if g == 0 { 1.0 } else { -1.0 };
        for (t, x) in terms.iter_mut().zip(team0_terms) {
            *t += sign * x;
        }
    }
    Ok(PairResult {
        pair: i,
        deal_seed,
        scores,
        rounds,
        aivat: spec.aivat.map(|_| PairAivat {
            deal: terms[0],
            draw: terms[1],
            action: terms[2],
        }),
    })
}

/// Version and settings from every engine, then the header, which each
/// engine must echo. The engines of one side must report the same settings,
/// and every set of engines the same settings as the first set.
fn handshake(
    spec: &Spec,
    sets: &mut [Vec<Engine>],
    per_side: usize,
    header: &mut String,
) -> Result<(), (Fault, String)> {
    let mut first: Vec<Settings> = Vec::new();
    for engines in sets.iter_mut() {
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
        if first.is_empty() {
            first = reported;
        } else if let Some(k) = (0..first.len()).find(|&k| reported[k] != first[k]) {
            return Err((
                Fault::Protocol,
                format!(
                    "{}: settings differ from the first set's: {:?}",
                    engines[k].name, reported[k]
                ),
            ));
        }
    }
    *header = spec.header(&[first[0].clone(), first[per_side].clone()]);
    for e in sets.iter_mut().flatten() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aivat_row_keeps_the_term_totals() {
        // Terms divided by the pair's 8 rounds biased full-game results (Q12).
        let p = PairResult {
            pair: 0,
            deal_seed: 0,
            scores: [[100, 60], [40, 100]],
            rounds: [3, 5],
            aivat: Some(PairAivat {
                deal: 8.0,
                draw: 0.0,
                action: -4.0,
            }),
        };
        assert_eq!(p.aivat_row(), Some((-20.0 / 8.0, [8.0, 0.0, -4.0])));
    }

    fn won(i: u64) -> PairResult {
        PairResult {
            pair: i,
            deal_seed: i,
            scores: [[100, 0], [100, 0]],
            rounds: [1, 1],
            aivat: None,
        }
    }

    /// `schedule` on `n` workers whose pair i takes `delay(i)` ms, costs one
    /// move, and fails when `fails(i)`, with a stop after pair `stop_at`:
    /// the pairs taken, the moves counted, and the outcome.
    fn scheduled(
        n: usize,
        pairs: u64,
        delay: impl Fn(u64) -> u64 + Sync,
        fails: impl Fn(u64) -> bool + Sync,
        stop_at: Option<u64>,
    ) -> (Vec<u64>, u64, Result<(), (Fault, String)>) {
        let mut workers = vec![(); n];
        let play = |_: &mut (), i: u64| -> Played {
            std::thread::sleep(Duration::from_millis(delay(i)));
            let stats = PairStats {
                moves: 1,
                ..PairStats::default()
            };
            if fails(i) {
                (Err((Fault::Crash, format!("pair {i}"))), stats)
            } else {
                (Ok(won(i)), stats)
            }
        };
        let mut report = Report::default();
        let outcome = schedule(
            pairs,
            &mut workers,
            &play,
            &mut |p| stop_at == Some(p.pair),
            &mut report,
        );
        let taken = report.pairs.iter().map(|p| p.pair).collect();
        (taken, report.moves, outcome)
    }

    #[test]
    fn schedule_takes_the_pairs_in_index_order() {
        for n in 1..=4 {
            // Even pairs finish late, so results arrive out of order.
            let (taken, moves, outcome) =
                scheduled(n, 12, |i| if i % 2 == 0 { 20 } else { 0 }, |_| false, None);
            assert_eq!(taken, (0..12).collect::<Vec<_>>(), "{n} workers");
            assert_eq!(moves, 12, "{n} workers");
            assert!(outcome.is_ok(), "{n} workers");
        }
    }

    #[test]
    fn schedule_stops_at_the_same_pair_and_counts_no_pair_past_it() {
        for n in 1..=4 {
            let (taken, moves, outcome) = scheduled(n, 100, |i| (i * 7) % 5, |_| false, Some(9));
            assert_eq!(taken, (0..10).collect::<Vec<_>>(), "{n} workers");
            assert_eq!(moves, 10, "{n} workers: a pair past the stop counted");
            assert!(outcome.is_ok(), "{n} workers");
        }
    }

    /// Waits until `flag` is set; panics after 10 s, so a test fails
    /// instead of hanging.
    fn wait_for(flag: &std::sync::atomic::AtomicBool) {
        let start = Instant::now();
        while !flag.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "a flag never came"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_fault_in_a_pair_past_the_stop_voids_the_match() {
        use std::sync::atomic::{AtomicBool, Ordering};
        // Pair 9, the stop, waits until pair 10 has started; pair 10 fails
        // only after the referee has taken pair 9 and stopped.
        let (started, stopped) = (AtomicBool::new(false), AtomicBool::new(false));
        let mut workers = vec![(); 2];
        let play = |_: &mut (), i: u64| -> Played {
            let stats = PairStats {
                moves: 1,
                ..PairStats::default()
            };
            match i {
                9 => wait_for(&started),
                10 => {
                    started.store(true, Ordering::SeqCst);
                    wait_for(&stopped);
                    return (Err((Fault::Crash, "pair 10".into())), stats);
                }
                _ => {}
            }
            (Ok(won(i)), stats)
        };
        let mut stop = |p: &PairResult| {
            let done = p.pair == 9;
            stopped.store(done, Ordering::SeqCst);
            done
        };
        let mut report = Report::default();
        let outcome = schedule(100, &mut workers, &play, &mut stop, &mut report);
        let taken: Vec<u64> = report.pairs.iter().map(|p| p.pair).collect();
        assert_eq!(taken, (0..10).collect::<Vec<_>>());
        assert_eq!(
            report.moves, 11,
            "the faulted pair's cost counts, and no other"
        );
        assert_eq!(outcome.expect_err("a fault").1, "pair 10");
    }

    #[test]
    fn the_lowest_fault_comes_after_every_pair_below_it() {
        // Pair 5 fails late and pair 6 at once: pairs 0 to 4 still count,
        // and the fault is pair 5's, as with one worker.
        for n in 1..=4 {
            let (taken, moves, outcome) = scheduled(
                n,
                20,
                |i| match i {
                    3 => 40,
                    5 => 60,
                    _ => 0,
                },
                |i| i == 5 || i == 6,
                None,
            );
            assert_eq!(taken, (0..5).collect::<Vec<_>>(), "{n} workers");
            assert_eq!(moves, 6, "{n} workers");
            assert_eq!(outcome.expect_err("a fault").1, "pair 5", "{n} workers");
        }
    }

    #[test]
    #[should_panic(expected = "a scoped thread panicked")]
    fn a_worker_panic_ends_the_match() {
        let mut workers = vec![(); 3];
        let play = |_: &mut (), i: u64| -> Played {
            assert_ne!(i, 2, "a worker panic");
            std::thread::sleep(Duration::from_millis(5));
            (Ok(won(i)), PairStats::default())
        };
        let _ = schedule(
            50,
            &mut workers,
            &play,
            &mut |_| false,
            &mut Report::default(),
        );
    }

    #[test]
    #[should_panic(expected = "a referee panic")]
    fn a_referee_panic_ends_the_match() {
        let mut workers = vec![(); 3];
        let play = |_: &mut (), i: u64| -> Played { (Ok(won(i)), PairStats::default()) };
        let mut stop = |p: &PairResult| {
            assert_ne!(p.pair, 3, "a referee panic");
            false
        };
        let _ = schedule(50, &mut workers, &play, &mut stop, &mut Report::default());
    }

    #[test]
    fn check_quiet_finds_a_line_out_of_turn_and_an_exit() {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("dominofish-check-quiet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the temporary directory");
        let engine = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the script");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("make the script executable");
            let side = Side::new(path.to_str().expect("a UTF-8 path")).expect("the side");
            Engine::spawn(&side, None, name.into()).expect("spawn the script")
        };
        let mut stray = engine("stray", "echo 'move pass'\nsleep 5");
        let mut exited = engine("exited", "exit 0");
        let mut quiet = engine("quiet", "sleep 5");
        // The line and the exit arrive when they arrive: poll for them.
        let fault_of = |e: &mut Engine| {
            let start = Instant::now();
            loop {
                if let Err((kind, _)) = e.check_quiet() {
                    return kind;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "{} gave no fault",
                    e.name
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        assert_eq!(fault_of(&mut stray), Fault::Protocol);
        assert_eq!(fault_of(&mut exited), Fault::Crash);
        assert!(quiet.check_quiet().is_ok());
        for e in [stray, exited, quiet] {
            e.stop();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

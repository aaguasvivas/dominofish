//! Solver commands (M3): `dominofish bench` solves the committed position
//! set (fixtures/bench.txt) and prints "Bench: <nodes>"; `dominofish
//! solve-stats` measures solve time and nodes by tiles left; `dominofish
//! bench-positions` writes a position set.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use dominofish_core::position;
use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, Mode, MoveList, Phase, State};
use dominofish_core::solver::{self, Table, tiles_left};

/// The default node cap of one solve (docs/PLAN.md D15).
pub const NODE_CAP: u64 = 10_000_000;
/// The default table: 2^20 entries of 16 bytes (16 MB).
pub const TABLE_BITS: u32 = 20;
/// Table sizes stay within the 256 MB per engine target (BUILD.md, hard
/// constraint 10): 2^24 entries.
pub const MAX_TABLE_BITS: u32 = 24;

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The committed bench positions, built into the binary, so a binary's
/// Bench depends on that binary alone (the harness runs it to fill a
/// searching engine's Bench in the match header).
pub const BENCH_POSITIONS: &str = include_str!("../../../fixtures/bench.txt");

/// The positions of a bench file, or of the built-in set when `path` is
/// None: one state text per line; blank lines and lines starting with #
/// are skipped.
pub fn read_positions(path: Option<&str>) -> Result<Vec<State>, String> {
    let (text, path) = match path {
        Some(p) => (
            std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?,
            p,
        ),
        None => (BENCH_POSITIONS.to_string(), "fixtures/bench.txt (built in)"),
    };
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let state = position::from_text(line)
            .ok_or_else(|| format!("{path}:{}: not a state text line", i + 1))?;
        if state.phase != Phase::Playing {
            return Err(format!("{path}:{}: the round is over", i + 1));
        }
        out.push(state);
    }
    Ok(out)
}

/// Solves every position with a cleared table and prints one line each,
/// then the totals; the last line is "Bench: <nodes>".
pub fn bench(path: Option<&str>, cap: u64, bits: u32) -> Result<String, String> {
    let positions = read_positions(path)?;
    let mut table = Table::new(bits);
    let mut out = format!(
        "DominoFish {} bench   {} positions   node cap {} per solve   table {} MB\n\n",
        env!("CARGO_PKG_VERSION"),
        positions.len(),
        thousands(cap),
        table.bytes() >> 20
    );
    out.push_str("   #  mode  tiles left  value         nodes\n");
    let (mut total, mut capped) = (0u64, 0u32);
    let start = Instant::now();
    for (i, p) in positions.iter().enumerate() {
        let mut state = *p;
        table.clear();
        let s = solver::solve(&mut state, Some(&mut table), cap);
        total += s.nodes;
        capped += u32::from(s.capped);
        let _ = writeln!(
            out,
            "  {:>2}  {}   {:>10}  {:>+5}  {:>12}{}",
            i + 1,
            state.mode.name(),
            tiles_left(&state),
            s.value,
            thousands(s.nodes),
            if s.capped { "  capped" } else { "" }
        );
    }
    let ms = start.elapsed().as_millis().max(1);
    let _ = write!(
        out,
        "\n===========================\nTotal time (ms) : {ms}\nNodes searched  : {}\nNodes/second    : {}\nCapped solves   : {capped}\nBench: {total}\n",
        thousands(total),
        thousands(total * 1000 / ms as u64),
    );
    Ok(out)
}

/// Round 1 after its automatic tile and round 2 at its lead, of `games`
/// seeded random games per mode: full-round positions for the bench.
pub fn bench_positions(games: u64, seed: u64) -> String {
    let mut out = format!(
        "# DominoFish bench positions: dominofish bench-positions --games {games} --seed {seed}\n# Round 1 after its automatic tile and round 2 at its lead, per game and mode (state text, docs/FORMATS.md).\n"
    );
    let mut moves = MoveList::new();
    for mode in [Mode::TwoVsTwo, Mode::OneVsOne] {
        for g in 0..games {
            let game = derive(seed, g + if mode == Mode::OneVsOne { 1000 } else { 0 });
            let mut rng = Rng::new(game);
            let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
            out.push_str(&position::to_text(&state));
            out.push('\n');
            while state.phase == Phase::Playing {
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                state.play(mv);
            }
            if state.phase == Phase::RoundOver {
                let next = state.next_round(&Deal::shuffled(&mut rng));
                out.push_str(&position::to_text(&next));
                out.push('\n');
            }
        }
    }
    out
}

fn quantile(sorted: &[u64], q: f64) -> u64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Solves every position of the first two rounds of `games` seeded random
/// games, each with a cleared table, and reports time and nodes by tiles
/// left, and the share of round-start solves that hit the cap.
pub fn stats(mode: Mode, games: u64, seed: u64, cap: u64, bits: u32) -> String {
    let mut table = Table::new(bits);
    let mut moves = MoveList::new();
    // Per tiles left: (nanoseconds, nodes, capped).
    let mut by_left: BTreeMap<u32, Vec<(u64, u64, bool)>> = BTreeMap::new();
    let start = Instant::now();
    for g in 0..games {
        let mut rng = Rng::new(derive(seed, g));
        let (mut state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut rng));
        let mut rounds = 1;
        loop {
            match state.phase {
                Phase::Finished => break,
                Phase::RoundOver if rounds == 2 => break,
                Phase::RoundOver => {
                    state = state.next_round(&Deal::shuffled(&mut rng));
                    rounds += 1;
                }
                Phase::Playing => {
                    table.clear();
                    let t = Instant::now();
                    let s = solver::solve(&mut state, Some(&mut table), cap);
                    let ns = t.elapsed().as_nanos() as u64;
                    by_left
                        .entry(tiles_left(&state))
                        .or_default()
                        .push((ns, s.nodes, s.capped));
                    state.legal_moves(&mut moves);
                    let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                    state.play(mv);
                }
            }
        }
    }
    let mut out = format!(
        "DominoFish solve-stats   mode {}   games {games}   seed {seed}   node cap {}   table {} MB   one thread\n\n",
        mode.name(),
        thousands(cap),
        table.bytes() >> 20
    );
    out.push_str("| Tiles left | Solves | Time ms median | p90 | max | Nodes median | p90 | max | Capped |\n|---|---|---|---|---|---|---|---|---|\n");
    let (mut solves, mut round_starts, mut round_start_capped, mut capped_all) = (0, 0, 0, 0);
    for (left, v) in &by_left {
        let mut ns: Vec<u64> = v.iter().map(|x| x.0).collect();
        let mut nodes: Vec<u64> = v.iter().map(|x| x.1).collect();
        ns.sort_unstable();
        nodes.sort_unstable();
        let capped = v.iter().filter(|x| x.2).count();
        solves += v.len();
        capped_all += capped;
        if *left >= 27 {
            round_starts += v.len();
            round_start_capped += capped;
        }
        let ms = |x: u64| format!("{:.3}", x as f64 / 1e6);
        let _ = writeln!(
            out,
            "| {left} | {} | {} | {} | {} | {} | {} | {} | {capped} |",
            v.len(),
            ms(quantile(&ns, 0.5)),
            ms(quantile(&ns, 0.9)),
            ms(*ns.last().unwrap()),
            quantile(&nodes, 0.5),
            quantile(&nodes, 0.9),
            nodes.last().unwrap()
        );
    }
    let _ = write!(
        out,
        "\nsolve-stats: mode {} solves {solves} capped {capped_all} ({:.2}%); round starts (27 or 28 tiles left) {round_starts}, capped {round_start_capped} ({:.2}%); wall {:.1} s\n",
        mode.name(),
        100.0 * capped_all as f64 / solves.max(1) as f64,
        100.0 * round_start_capped as f64 / round_starts.max(1) as f64,
        start.elapsed().as_secs_f64()
    );
    out
}

//! `dominofish capi-fuzz`: the Rust side of the lockstep differential fuzzer
//! (M1-07). Spawns tools/capi-oracle/fuzz.mjs, which plays seeded rounds
//! through Capi's engine, and replays every move through the referee,
//! comparing Capi's canonical and raw legal-move sets before each move and a digest of
//! the canonical state and event after it (docs/FORMATS.md, Differential
//! fuzzer). Runs from the repository root.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, Phase, State};
use dominofish_core::tile::Tile;

pub const PIN: &str = "1d8868a26ce930f478f98e64060c06b1d1a2c1f4";
const NODE: &str = "/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node";

/// The outcome: a summary line, and whether it passed. A missing Capi
/// checkout is a SKIP, which does not pass.
pub struct Outcome {
    pub summary: String,
    pub errors: Vec<String>,
    pub passed: bool,
}

fn digest(words: &[u32]) -> u32 {
    let mut h = 0x811c_9dc5u32;
    for &w in words {
        h = (h ^ w).wrapping_mul(0x0100_0193);
    }
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^ (h >> 13)
}

fn state_words(s: &State, out: &mut Vec<u32>) {
    let phase = match s.phase {
        Phase::Playing => 0,
        Phase::RoundOver => 1,
        Phase::Finished => 2,
    };
    let hands = match s.mode {
        Mode::TwoVsTwo => s.hands,
        Mode::OneVsOne => [s.hands[0], s.hands[1], 0, 0],
    };
    let empty = s.board == 0;
    out.extend([
        phase,
        u32::from(s.scores[0]),
        u32::from(s.scores[1]),
        u32::from(s.turn),
        u32::from(s.starter),
        u32::from(s.last_played_by),
        u32::from(s.passes),
    ]);
    out.extend(hands);
    out.extend([
        s.board,
        if empty { 7 } else { u32::from(s.left) },
        if empty { 7 } else { u32::from(s.right) },
        s.boneyard().len() as u32,
    ]);
}

fn event_words(e: Option<Event>, out: &mut Vec<u32>) {
    let w = |x: u8| u32::from(x);
    match e {
        None => out.push(0),
        Some(Event::Domino { seat, points }) => out.extend([1, w(seat), u32::from(points)]),
        Some(Event::Capicua { seat, points }) => out.extend([2, w(seat), u32::from(points)]),
        Some(Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        }) => out.extend([
            3,
            w(blocker),
            w(rival),
            u32::from(blocker_pips),
            u32::from(rival_pips),
            w(winner),
            u32::from(points),
        ]),
        Some(Event::PaseCorrido { team }) => out.extend([4, team as u32]),
        Some(Event::PaseSalida { team }) => out.extend([5, team as u32]),
    }
}

/// Rust's canonical legal set in the fuzzer's encoding: bit 2 * tile + end
/// for plays, 56 for draw, 57 for pass.
fn legal_mask(s: &State) -> u64 {
    let mut moves = MoveList::new();
    s.legal_moves(&mut moves);
    moves.as_slice().iter().fold(0u64, |m, mv| {
        m | 1u64
            << match *mv {
                Move::Play { tile, end } => 2 * tile.index() as u32 + u32::from(end == End::Right),
                Move::Draw => 56,
                Move::Pass => 57,
            }
    })
}

/// Rust's raw legal set: every move `is_legal` accepts, in the same encoding.
/// A double on an empty board has one placement, so only its left move is
/// listed there (Capi's intents for it all map to that move).
fn raw_mask(s: &State) -> u64 {
    let mut m = 0u64;
    for i in 0..28u8 {
        let tile = Tile::from_index(i);
        for (end, bit) in [(End::Left, 0), (End::Right, 1)] {
            if s.board == 0 && tile.is_double() && end == End::Right {
                continue;
            }
            if s.is_legal(Move::Play { tile, end }) {
                m |= 1u64 << (2 * u32::from(i) + bit);
            }
        }
    }
    if s.is_legal(Move::Draw) {
        m |= 1 << 56;
    }
    if s.is_legal(Move::Pass) {
        m |= 1 << 57;
    }
    m
}

/// A move code from fuzz.mjs: "<a><b><l|r>" (the tile as Capi's intent
/// orients it), "d", or "p"; mapped by docs/RULES.md (Capi intents to moves).
fn parse_move(s: &State, code: &str) -> Move {
    match code {
        "d" => Move::Draw,
        "p" => Move::Pass,
        _ => {
            let b = code.as_bytes();
            let (x, y) = (b[0] - b'0', b[1] - b'0');
            let end = if s.board == 0 {
                if x <= y { End::Left } else { End::Right }
            } else if b[2] == b'l' {
                End::Left
            } else {
                End::Right
            };
            Move::Play {
                tile: Tile::new(x, y),
                end,
            }
        }
    }
}

/// The rule paths docs/RULES.md (Rule paths) requires above zero in `mode`.
fn required_paths(mode: Mode) -> Result<Vec<String>, String> {
    let rules =
        std::fs::read_to_string("docs/RULES.md").map_err(|e| format!("docs/RULES.md: {e}"))?;
    let table = rules
        .split("## Rule paths")
        .nth(1)
        .ok_or("docs/RULES.md has no Rule paths section")?;
    let want = match mode {
        Mode::OneVsOne => "1v1",
        Mode::TwoVsTwo => "2v2",
    };
    Ok(table
        .lines()
        .take_while(|l| !l.starts_with("## "))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            (cells.len() > 3 && cells[2].split(", ").any(|m| m == want))
                .then(|| cells[1].to_string())
        })
        .collect())
}

pub fn run(mode: Mode, rounds: u64, seed: u64) -> Result<Outcome, String> {
    let capi_dir =
        std::env::var("CAPI_DIR").unwrap_or_else(|_| "/Users/Adelson/Desktop/personal/capi".into());
    let has_pin = Command::new("/usr/bin/git")
        .args([
            "-C",
            &capi_dir,
            "cat-file",
            "-e",
            &format!("{PIN}^{{commit}}"),
        ])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !has_pin {
        return Ok(Outcome {
            summary: format!("capi-fuzz: SKIP CAPI_DIR={capi_dir} does not hold Capi commit {PIN}"),
            errors: vec![],
            passed: false,
        });
    }
    let built =
        std::fs::read_to_string("tools/capi-oracle/build/PIN").is_ok_and(|p| p.trim() == PIN);
    if !built {
        let status = Command::new("tools/capi-oracle/build.sh")
            .env("CAPI_DIR", &capi_dir)
            .stdout(Stdio::null())
            .status()
            .map_err(|e| format!("tools/capi-oracle/build.sh: {e}"))?;
        if !status.success() {
            return Err("tools/capi-oracle/build.sh failed".into());
        }
    }
    if !Path::new("tools/capi-oracle/fuzz.mjs").exists() {
        return Err("run capi-fuzz from the repository root".into());
    }
    let mode_arg = match mode {
        Mode::OneVsOne => "1v1",
        Mode::TwoVsTwo => "2v2",
    };
    let mut child = Command::new(NODE)
        .args([
            "tools/capi-oracle/fuzz.mjs",
            mode_arg,
            &rounds.to_string(),
            &seed.to_string(),
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{NODE}: {e}"))?;
    let reader = BufReader::with_capacity(1 << 20, child.stdout.take().expect("piped stdout"));

    let mut errors = Vec::new();
    let mut mismatches = 0u64;
    let (mut seen_rounds, mut moves, mut end_line) = (0u64, 0u64, None);
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut state: Option<State> = None;
    let mut round = String::new();
    let mut words = Vec::with_capacity(32);
    let fail = |errors: &mut Vec<String>, what: String| {
        if errors.len() < 10 {
            errors.push(what);
        }
    };
    for line in reader.lines() {
        let line = line.map_err(|e| format!("reading fuzz.mjs output: {e}"))?;
        let w: Vec<&str> = line.split(' ').collect();
        match w[0] {
            "R" => {
                seen_rounds += 1;
                round = format!("seed {seed} round {}", w[1]);
                let target: u16 = w[6].parse().map_err(|_| "bad target")?;
                let mut tiles = [Tile::from_index(0); 28];
                for (k, v) in w[7].split(',').enumerate() {
                    tiles[k] = Tile::from_index(v.parse().map_err(|_| "bad tile")?);
                }
                let deal = Deal::from_tiles(tiles).ok_or("bad deal")?;
                let starter: u8 = w[3].parse().map_err(|_| "bad seat")?;
                let s = if w[2] == "r1" {
                    State::first_round(mode, target, &deal).0
                } else {
                    let scores = [
                        w[4].parse().map_err(|_| "bad score")?,
                        w[5].parse().map_err(|_| "bad score")?,
                    ];
                    State::dealt(mode, target, scores, &deal, starter)
                };
                if s.starter != starter {
                    mismatches += 1;
                    fail(
                        &mut errors,
                        format!(
                            "ERROR capi-fuzz {round}: opener want {starter} got {}",
                            s.starter
                        ),
                    );
                    state = None;
                } else {
                    state = Some(s);
                }
            }
            "P" => {
                counts.insert(w[1].to_string(), w[2].parse().unwrap_or(0));
            }
            "END" => end_line = Some(line.clone()),
            code => {
                moves += 1;
                let Some(s) = state.as_mut() else { continue };
                let capi_set = u64::from_str_radix(w[1], 16).unwrap_or(u64::MAX)
                    | u64::from_str_radix(w[2], 16).unwrap_or(u64::MAX) << 32;
                let rust_set = legal_mask(s);
                let capi_raw = u64::from_str_radix(w[3], 16).unwrap_or(u64::MAX)
                    | u64::from_str_radix(w[4], 16).unwrap_or(u64::MAX) << 32;
                let rust_raw = raw_mask(s);
                if capi_raw != rust_raw {
                    mismatches += 1;
                    fail(
                        &mut errors,
                        format!(
                            "ERROR capi-fuzz {round} move {code}: raw legal set want {capi_raw:x} got {rust_raw:x}"
                        ),
                    );
                    state = None;
                    continue;
                }
                if capi_set != rust_set {
                    mismatches += 1;
                    fail(
                        &mut errors,
                        format!(
                            "ERROR capi-fuzz {round} move {code}: legal set want {capi_set:x} got {rust_set:x}"
                        ),
                    );
                    state = None;
                    continue;
                }
                let mv = parse_move(s, code);
                if !s.is_legal(mv) {
                    mismatches += 1;
                    fail(
                        &mut errors,
                        format!(
                            "ERROR capi-fuzz {round}: Capi accepted {code}, Rust rejects {mv:?}"
                        ),
                    );
                    state = None;
                    continue;
                }
                let event = s.play(mv).event;
                words.clear();
                state_words(s, &mut words);
                event_words(event, &mut words);
                let got = digest(&words);
                if u32::from_str_radix(w[5], 16).ok() != Some(got) {
                    mismatches += 1;
                    fail(
                        &mut errors,
                        format!(
                            "ERROR capi-fuzz {round} move {code}: state or event differs (digest want {} got {got:x})",
                            w[5]
                        ),
                    );
                    state = None;
                }
            }
        }
    }
    let status = child.wait().map_err(|e| format!("fuzz.mjs: {e}"))?;
    if !status.success() {
        return Err(format!("fuzz.mjs exited {status}"));
    }
    let end = end_line.ok_or("fuzz.mjs stopped before its END line")?;
    if end != format!("END {rounds} {moves}") || seen_rounds != rounds {
        return Err(format!(
            "stream ended early: {end}, {seen_rounds} rounds and {moves} moves read"
        ));
    }
    let required = required_paths(mode)?;
    let missing: Vec<&String> = required
        .iter()
        .filter(|p| counts.get(*p).copied().unwrap_or(0) == 0)
        .collect();
    for p in &missing {
        fail(
            &mut errors,
            format!("ERROR capi-fuzz rule path {p} not reached in {mode_arg}"),
        );
    }
    let fewest = required
        .iter()
        .map(|p| (counts.get(p).copied().unwrap_or(0), p))
        .min()
        .map(|(n, p)| format!("{p} {n}"))
        .unwrap_or_default();
    let passed = mismatches == 0 && missing.is_empty();
    Ok(Outcome {
        summary: format!(
            "capi-fuzz: {} mode {mode_arg} seed {seed} rounds {rounds} moves {moves} mismatches {mismatches}; rule paths {} of {} above zero (fewest {fewest})",
            if passed { "PASS" } else { "FAIL" },
            required.len() - missing.len(),
            required.len()
        ),
        errors,
        passed,
    })
}

//! `dominofish report`: builds docs/eval/REPORT.md from the match result
//! files under docs/eval/results/ (docs/FORMATS.md, Match results), one
//! section per run directory, in the strength-claim format of BUILD.md, with
//! the directory's note.md, if any, after its settings. Every
//! number is recomputed from the pair lines: the 95% intervals, and the SPRT
//! replayed in pair order with the bounds the run used.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::stats::{Decision, Sprt, crossfit, summary, wilson};

struct Row {
    file: PathBuf,
    header: String,
    seeds: String,
    run: String,
    /// The `# time` line: each side's reply time per move.
    time: String,
    /// Each pair's AIVAT term totals over both games (deal, draw, action),
    /// when the match ran with --aivat.
    aivat: Vec<[f64; 3]>,
    margins: Vec<f64>,
    per_round: Vec<f64>,
    wins: Vec<f64>,
    /// The SPRT's bounds, and whether it ran on the game-win share.
    sprt: Option<(f64, f64, bool)>,
}

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split(' ')
        .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
}

fn read_row(path: &Path) -> Result<Row, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut row = Row {
        file: path.to_path_buf(),
        header: String::new(),
        seeds: String::new(),
        run: String::new(),
        time: String::new(),
        aivat: vec![],
        margins: vec![],
        per_round: vec![],
        wins: vec![],
        sprt: None,
    };
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("# header ") {
            row.header = h.to_string();
        } else if let Some(r) = line.strip_prefix("# seeds ") {
            row.seeds = r.to_string();
        } else if let Some(r) = line.strip_prefix("# run ") {
            row.run = r.to_string();
        } else if let Some(t) = line.strip_prefix("# time ") {
            row.time = t.to_string();
        } else if line.starts_with("# fault") {
            return Err(format!(
                "{}: the run has a fault and is void",
                path.display()
            ));
        } else if let Some(s) = line.strip_prefix("# summary ") {
            let on = [
                "sprt on the pair margin, H0 ",
                "sprt on the game-win share, H0 ",
            ]
            .iter()
            .enumerate()
            .find_map(|(k, p)| s.find(p).map(|i| (i, k == 1)));
            if let Some((i, wins)) = on {
                let w: Vec<&str> = s[i..].split(' ').collect();
                // "sprt on the pair margin, H0 <mu0> H1 <mu1>, ...", or on
                // "the game-win share" with the same fields.
                let (h0, h1) = (
                    w.get(6).and_then(|x| x.parse::<f64>().ok()),
                    w.get(8)
                        .and_then(|x| x.trim_end_matches(',').parse::<f64>().ok()),
                );
                row.sprt = h0
                    .zip(h1)
                    .filter(|&(a, b)| a.is_finite() && b.is_finite() && b > a)
                    .map(|(a, b)| (a, b, wins));
            }
        } else if line.starts_with("aivat ") {
            let bad = || format!("{}: bad aivat line: {line}", path.display());
            let w: Vec<&str> = line.split(' ').collect();
            if w.len() != 8 || w[2] != "deal" || w[4] != "draw" || w[6] != "action" {
                return Err(bad());
            }
            let num = |x: &str| x.parse::<f64>().map_err(|_| bad());
            row.aivat.push([num(w[3])?, num(w[5])?, num(w[7])?]);
        } else if line.starts_with("pair ") {
            let bad = || format!("{}: bad pair line: {line}", path.display());
            let w: Vec<&str> = line.split(' ').collect();
            if w.len() != 14 || w[8] != "rounds" || w[10] != "margin" || w[12] != "wins" {
                return Err(bad());
            }
            let margin: f64 = w[11].parse().map_err(|_| bad())?;
            let rounds: f64 = w[9]
                .split(',')
                .map(|x| x.parse::<f64>().map_err(|_| bad()))
                .sum::<Result<f64, String>>()?;
            let wins: f64 = w[13].parse().map_err(|_| bad())?;
            if rounds <= 0.0 {
                return Err(bad());
            }
            row.margins.push(margin);
            row.per_round.push(margin / rounds);
            row.wins.push(wins / 2.0);
        }
    }
    if row.header.is_empty() || row.margins.is_empty() {
        return Err(format!("{}: no header or no pairs", path.display()));
    }
    if !row.aivat.is_empty() && row.aivat.len() != row.margins.len() {
        return Err(format!(
            "{}: {} aivat lines for {} pairs",
            path.display(),
            row.aivat.len(),
            row.margins.len()
        ));
    }
    Ok(row)
}

/// The bot of an engine command: the word after --bot.
fn bot_of(cmd: &str) -> &str {
    let words: Vec<&str> = cmd.split('+').collect();
    words
        .iter()
        .position(|&w| w == "--bot")
        .and_then(|i| words.get(i + 1))
        .copied()
        .unwrap_or(cmd)
}

/// One side's identity and settings. R1's committed files predate per-side
/// settings and the seeds line: their header holds one budgets, threads,
/// and tables for both sides, and the deal seeds.
fn side_text(header: &str, side: &str) -> String {
    let get = |key: &str| {
        field(header, &format!("{side}.{key}"))
            .or_else(|| field(header, key))
            .unwrap_or("-")
    };
    let view = match field(header, &format!("{side}.view")) {
        Some("omniscient") => ", view omniscient (sees every hand: cheating)",
        _ => "",
    };
    format!(
        "commit {}, Bench {}, binary hash {}, budgets {}, threads {}, tables {}{view}",
        get("commit"),
        get("bench"),
        get("hash"),
        get("budgets"),
        get("threads"),
        get("tables")
    )
}

/// A side's settings over a run's rows: one text when every row has the
/// same, else each distinct text with the rows that have it (a run can play
/// PIMC in some rows and baseline bots in others).
fn sides_text(rows: &[Row], ids: &[u32], side: &str) -> String {
    let mut groups: Vec<(String, Vec<u32>)> = Vec::new();
    for (r, &id) in rows.iter().zip(ids) {
        let text = side_text(&r.header, side);
        match groups.iter_mut().find(|(t, _)| *t == text) {
            Some((_, g)) => g.push(id),
            None => groups.push((text, vec![id])),
        }
    }
    if groups.len() == 1 {
        return groups.remove(0).0;
    }
    groups
        .iter()
        .map(|(text, g)| format!("{text} in rows {}", row_list(g)))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Row numbers in order as text, with runs of three or more as "a to b".
fn row_list(ids: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < ids.len() {
        let mut j = i;
        while j + 1 < ids.len() && ids[j + 1] == ids[j] + 1 {
            j += 1;
        }
        if j >= i + 2 {
            parts.push(format!("{} to {}", ids[i], ids[j]));
        } else {
            parts.extend(ids[i..=j].iter().map(u32::to_string));
        }
        i = j + 1;
    }
    parts.join(", ")
}

/// The row number in a result file's name, row-<k>-...
fn row_number(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_string_lossy().to_string();
    name.split('-').nth(1)?.parse().ok()
}

/// The table label of a side: its bot, marked when it cheats, with PIMC's
/// worlds per move.
fn side_label(header: &str, side: &str) -> String {
    let bot = bot_of(field(header, &format!("{side}.cmd")).unwrap_or("-"));
    let worlds = field(header, &format!("{side}.budgets"))
        .and_then(|b| b.split(',').find_map(|x| x.strip_prefix("worlds:")));
    if field(header, &format!("{side}.view")) == Some("omniscient") {
        format!("{bot} (cheating)")
    } else if let Some(w) = worlds {
        format!("{bot} ({w} worlds)")
    } else {
        bot.to_string()
    }
}

/// Side A's reply time per move, "mean / max" in milliseconds, from the
/// `# time` line (- before it existed).
fn time_text(time: &str) -> String {
    match field(time, "a").map(|t| t.split(',').collect::<Vec<_>>()) {
        Some(v) if v.len() == 3 => format!("{} / {}", v[0], v[1]),
        _ => "-".into(),
    }
}

/// The report text for every run directory under `results`.
pub fn build(results: &Path) -> Result<String, String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(results)
        .map_err(|e| format!("{}: {e}", results.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut out = String::from(
        "# DominoFish evaluation report\n\nGenerated by `dominofish report` from docs/eval/results/; do not edit by hand. Every result here is self-play between DominoFish's own bots and engines: relative strength only, not a measure against people. Each row is a match of duplicate pairs (one deal played twice with the sides swapping seats) on held-out deal seeds; A is the first side. Points per round are side A's points minus side B's, per round; the game-win rate is side A's share of games won. Unit round plays round 1 of each game only; unit game plays each game to its target. Intervals are 95%: a t interval over pairs for points per round, and a Wilson score interval over the games for the game-win rate. The SPRT is on the pair margin (H0 and H1 are mean pair margins in points, alpha = beta = 0.05), or, where its bounds say wins, on each pair's share of games won by A; its outcome is the first boundary crossing, and a run that never crosses is inconclusive. Peak RSS is the maximum resident set size that /usr/bin/time -l reports for the match command; it is not a total over the engine processes. Milliseconds per move run from the referee's go to side A's move (a record, not a budget; - for runs before it was recorded). The corrected result subtracts AIVAT control variates (docs/PLAN.md D19) with coefficients fitted on the other half of the pairs, which keeps its mean; - for runs without --aivat.\n",
    );
    for dir in dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "txt"))
            .collect();
        files.sort_by_key(|p| row_number(p).unwrap_or(u32::MAX));
        let rows: Vec<Row> = files
            .iter()
            .map(|f| read_row(f))
            .collect::<Result<_, _>>()?;
        let Some(first) = rows.first() else { continue };
        let ids: Vec<u32> = files
            .iter()
            .enumerate()
            .map(|(i, p)| row_number(p).unwrap_or(i as u32))
            .collect();
        let mut dates: Vec<&str> = rows.iter().filter_map(|r| field(&r.run, "date")).collect();
        dates.sort_unstable();
        let date = match (dates.first(), dates.last()) {
            (Some(lo), Some(hi)) if lo == hi => lo.to_string(),
            (Some(lo), Some(hi)) => format!("{lo} to {hi}"),
            _ => "-".to_string(),
        };
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let run_id = field(&first.run, "run").unwrap_or("-");
        let loads: Vec<f64> = rows
            .iter()
            .filter_map(|r| field(&r.run, "load")?.parse().ok())
            .collect();
        let load = if loads.is_empty() {
            "-".to_string()
        } else {
            format!(
                "{} to {}",
                loads.iter().copied().fold(f64::INFINITY, f64::min),
                loads.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            )
        };
        let bases: Vec<u64> = rows
            .iter()
            .filter_map(|r| field(&r.seeds, "engines")?.parse().ok())
            .collect();
        let engine_seeds = match (bases.iter().min(), bases.iter().max()) {
            (Some(lo), Some(hi)) if lo == hi => lo.to_string(),
            (Some(lo), Some(hi)) => format!("{lo} to {hi} (one per row)"),
            _ => "- (seat seeds keyed by deal and seat)".to_string(),
        };
        let _ = write!(
            out,
            "\n## {name} ({run_id})\n\nPre-registered as {run_id} in docs/PLAN.md. Side A: {}. Side B: {}. Engine seed base: {engine_seeds}. Measured {} at a 1-minute load average of {load}.\n\n",
            sides_text(&rows, &ids, "a"),
            sides_text(&rows, &ids, "b"),
            date,
        );
        // note.md in a run directory: a later finding about that run, added
        // beside its committed result files instead of editing them.
        if let Ok(note) = std::fs::read_to_string(dir.join("note.md")) {
            let _ = write!(out, "{}\n\n", note.trim_end());
        }
        out.push_str("| Mode | Unit | Target | A | B | Pairs | A points per round [95% CI] | A game-win rate [95% CI] | SPRT (H0, H1) | Wall s | Peak RSS MB | Deal seeds | File | A ms per move (mean / max) | A corrected points per round [95% CI] | SD raw / corrected |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for r in &rows {
            let h = &r.header;
            let unit = field(h, "unit").unwrap_or("-");
            let pr = summary(&r.per_round);
            let win = if unit == "game" {
                let w = summary(&r.wins);
                let games = 2.0 * w.n as f64;
                let (lo, hi) = wilson(w.mean * games, games);
                format!("{:.3} [{lo:.3}, {hi:.3}]", w.mean)
            } else {
                "-".into()
            };
            let sprt = match r.sprt {
                Some((h0, h1, wins)) => {
                    let mut t = Sprt::new(h0, h1, 0.05, 0.05);
                    let (values, on) = if wins {
                        (&r.wins, "wins ")
                    } else {
                        (&r.margins, "")
                    };
                    values.iter().for_each(|&m| t.push(m));
                    match t.first_decision() {
                        Some((Decision::H1, n)) => format!("H1 at {n} ({on}{h0}, {h1})"),
                        Some((Decision::H0, n)) => format!("H0 at {n} ({on}{h0}, {h1})"),
                        None => format!("inconclusive ({on}{h0}, {h1})"),
                    }
                }
                None => "-".into(),
            };
            let (corrected, ratio) = if r.aivat.is_empty() {
                ("-".to_string(), "-".to_string())
            } else {
                // The term totals, as the harness takes them (D19, Q12).
                let rows: Vec<(f64, [f64; 3])> = r
                    .per_round
                    .iter()
                    .copied()
                    .zip(r.aivat.iter().copied())
                    .collect();
                let (c, _) = crossfit(&rows);
                let c = summary(&c);
                (
                    format!("{:.2} [{:.2}, {:.2}]", c.mean, c.lo, c.hi),
                    format!("{:.2}", pr.sd / c.sd),
                )
            };
            let rss = field(&r.run, "peak_rss_kb")
                .and_then(|k| k.parse::<f64>().ok())
                .map_or("-".into(), |k| format!("{:.1}", k / 1024.0));
            let _ = writeln!(
                out,
                "| {} | {unit} | {} | {} | {} | {} | {:.2} [{:.2}, {:.2}] | {win} | {sprt} | {} | {rss} | {} | {} | {} | {corrected} | {ratio} |",
                field(h, "mode").unwrap_or("-"),
                field(h, "target").unwrap_or("-"),
                side_label(h, "a"),
                side_label(h, "b"),
                r.margins.len(),
                pr.mean,
                pr.lo,
                pr.hi,
                field(&r.run, "wall_s").unwrap_or("-"),
                field(&r.seeds, "deals")
                    .or_else(|| field(h, "seeds"))
                    .unwrap_or("-"),
                r.file.file_name().unwrap().to_string_lossy(),
                time_text(&r.time)
            );
        }
        let _ = writeln!(
            out,
            "\nCommand of each row: `{}`, with the row's options; the files hold every pair.",
            field(&first.run, "command")
                .unwrap_or("-")
                .replace('+', " ")
        );
    }
    Ok(out)
}

/// Writes the report, or with `check` compares it with the file.
pub fn run(results: &Path, out: &Path, check: bool) -> Result<(String, bool), String> {
    let text = build(results)?;
    if check {
        let current = std::fs::read_to_string(out).unwrap_or_default();
        return Ok(if current == text {
            (format!("report: {} is current\n", out.display()), true)
        } else {
            (
                format!("ERROR {} is stale: run dominofish report\n", out.display()),
                false,
            )
        });
    }
    std::fs::write(out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((format!("report: wrote {}\n", out.display()), true))
}

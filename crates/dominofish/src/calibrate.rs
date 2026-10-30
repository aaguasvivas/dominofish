//! dominofish calibrate (M6-03; docs/PLAN.md T8): how well PIMC's win
//! probability predicts the game. Self-play games between copies of PIMC
//! with the win objective record, at every searched decision, the
//! probability the engine gives its team (the mean over the worlds of the
//! move it plays) and the score-only baseline, W at the start of the
//! decision's round; the game's winner gives each outcome. The Brier scores,
//! the Brier skill, their 95% intervals from bootstrap resamples of whole
//! games, and a reliability table follow from the decision lines
//! (`summary`). Games run on up to `threads` threads, each seeded by its
//! index alone, so the file is the same at any thread count.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use dominofish_core::equity::Equity;
use dominofish_core::pimc::Limits;
use dominofish_core::rng::Rng;
use dominofish_core::rules::{Mode, team};
use dominofish_core::solver::{Objective, WIN};

use crate::equity::{Work, play_game};

/// A prediction: its round (from 0) and ply, the mover's team, and the
/// engine's probability and the baseline's that the team wins, in
/// millionths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prediction {
    pub round: usize,
    pub ply: usize,
    pub team: usize,
    pub p: u32,
    pub base: u32,
}

/// A game's predictions and its winning team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub predictions: Vec<Prediction>,
    pub winner: usize,
}

/// A played game, or why its run is void.
type Played = Result<(Game, Work), String>;

/// Games 0 to `games` - 1 to `target`, on `threads` threads.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Mode,
    target: u16,
    games: u64,
    deal_base: u64,
    engine_base: u64,
    limits: &Limits,
    w: &Equity,
    threads: usize,
) -> Result<(Vec<Game>, Work), String> {
    let next = AtomicU64::new(0);
    let done: Mutex<BTreeMap<u64, Played>> = Mutex::new(BTreeMap::new());
    let millionths = |p: f64| (p * f64::from(WIN)).round().clamp(0.0, f64::from(WIN)) as u32;
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let g = next.fetch_add(1, Ordering::SeqCst);
                    if g >= games {
                        break;
                    }
                    let mut predictions = Vec::new();
                    let result = play_game(
                        mode,
                        target,
                        g,
                        deal_base,
                        engine_base,
                        limits,
                        Objective::Win(w),
                        &mut |d| {
                            let t = team(d.seat);
                            let base = w.win(
                                d.start_scores[t],
                                d.start_scores[1 - t],
                                team(d.leader) == t,
                            );
                            predictions.push(Prediction {
                                round: d.round,
                                ply: d.ply,
                                team: t,
                                p: d.value.round().clamp(0.0, f64::from(WIN)) as u32,
                                base: millionths(base),
                            });
                        },
                    )
                    .map(|(_, end, work)| {
                        (
                            Game {
                                predictions,
                                winner: team(end.round_winner),
                            },
                            work,
                        )
                    });
                    let failed = result.is_err();
                    done.lock().expect("the results lock").insert(g, result);
                    if failed {
                        next.store(games, Ordering::SeqCst);
                    }
                }
            });
        }
    });
    let mut out = Vec::new();
    let mut work = Work::default();
    for (_, result) in done.into_inner().expect("the results lock") {
        let (game, w) = result?;
        out.push(game);
        work.games += w.games;
        work.moves += w.moves;
        work.decisions += w.decisions;
        work.capped += w.capped;
    }
    Ok((out, work))
}

/// The calibration file (docs/FORMATS.md, Calibration): a game line and its
/// decision lines per game, the work, and the summary.
pub fn text(mode: Mode, target: u16, settings: &str, games: &[Game], work: &Work) -> String {
    let mut out = format!(
        "# calibration mode={} target={target} {settings}\n",
        mode.name()
    );
    for (g, game) in games.iter().enumerate() {
        let _ = writeln!(
            out,
            "game {g} winner {} decisions {}",
            game.winner,
            game.predictions.len()
        );
        for d in &game.predictions {
            let _ = writeln!(
                out,
                "d {g} {} {} team {} p {} base {}",
                d.round + 1,
                d.ply,
                d.team,
                d.p,
                d.base
            );
        }
    }
    let _ = writeln!(
        out,
        "# work games={} moves={} decisions={} capped={}",
        work.games, work.moves, work.decisions, work.capped
    );
    out.push_str(&summary(games));
    out
}

/// The games of a calibration file.
pub fn parse(text: &str) -> Result<Vec<Game>, String> {
    let mut games: Vec<Game> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("line {}: {line}", n + 1);
        let num = |s: &str| s.parse::<u64>().map_err(|_| bad());
        match w.first() {
            Some(&"game") => {
                if w.len() != 6 || w[2] != "winner" || num(w[1])? != games.len() as u64 {
                    return Err(bad());
                }
                games.push(Game {
                    predictions: Vec::new(),
                    winner: num(w[3])? as usize,
                });
            }
            Some(&"d") => {
                let g = games.last_mut().ok_or_else(bad)?;
                if w.len() != 10 || w[4] != "team" || w[6] != "p" || w[8] != "base" {
                    return Err(bad());
                }
                g.predictions.push(Prediction {
                    round: num(w[2])? as usize - 1,
                    ply: num(w[3])? as usize,
                    team: num(w[5])? as usize,
                    p: num(w[7])? as u32,
                    base: num(w[9])? as u32,
                });
            }
            _ => {}
        }
    }
    if games.is_empty() {
        return Err("the file holds no games".into());
    }
    Ok(games)
}

/// Brier scores of the engine and the baseline over `games`, and the skill.
fn brier(games: &[&Game]) -> (f64, f64, f64) {
    let (mut e, mut b, mut n) = (0.0, 0.0, 0.0);
    for g in games {
        for d in &g.predictions {
            let o = if d.team == g.winner { 1.0 } else { 0.0 };
            let p = f64::from(d.p) / f64::from(WIN);
            let q = f64::from(d.base) / f64::from(WIN);
            e += (p - o) * (p - o);
            b += (q - o) * (q - o);
            n += 1.0;
        }
    }
    let (e, b) = (e / n, b / n);
    (e, b, 1.0 - e / b)
}

/// The summary lines: the Brier scores and the skill with 95% intervals
/// from 1,000 bootstrap resamples of whole games (seed 1), and the
/// reliability table in ten bins of the engine's prediction.
pub fn summary(games: &[Game]) -> String {
    let all: Vec<&Game> = games.iter().collect();
    let (e, b, s) = brier(&all);
    let mut rng = Rng::new(1);
    let mut draws: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for _ in 0..1000 {
        let sample: Vec<&Game> = (0..games.len())
            .map(|_| &games[rng.below(games.len() as u32) as usize])
            .collect();
        let (x, y, z) = brier(&sample);
        draws[0].push(x);
        draws[1].push(y);
        draws[2].push(z);
    }
    let ci = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[24], v[974])
    };
    let (el, eh) = ci(&mut draws[0]);
    let (bl, bh) = ci(&mut draws[1]);
    let (sl, sh) = ci(&mut draws[2]);
    let n: usize = games.iter().map(|g| g.predictions.len()).sum();
    let mut out = format!(
        "# summary games={} predictions={n}\n# brier {e:.4} [{el:.4}, {eh:.4}] base {b:.4} [{bl:.4}, {bh:.4}] skill {s:.4} [{sl:.4}, {sh:.4}] (95% intervals from 1000 bootstrap resamples of the games, seed 1)\n",
        games.len()
    );
    for k in 0..10u32 {
        let (mut count, mut sum, mut won) = (0u64, 0.0, 0u64);
        for g in games {
            for d in &g.predictions {
                if (d.p / 100_000).min(9) == k {
                    count += 1;
                    sum += f64::from(d.p) / f64::from(WIN);
                    won += u64::from(d.team == g.winner);
                }
            }
        }
        let (mean, share) = if count == 0 {
            ("-".to_string(), "-".to_string())
        } else {
            (
                format!("{:.3}", sum / count as f64),
                format!("{:.3}", won as f64 / count as f64),
            )
        };
        let _ = writeln!(
            out,
            "# bin {:.1}-{:.1} count {count} mean {mean} won {share}",
            f64::from(k) / 10.0,
            f64::from(k + 1) / 10.0
        );
    }
    out
}

//! `dominofish analyze`: PIMC's ranking of the legal moves of the seat on
//! turn at one point of a game record (M4-05; docs/PLAN.md D18). The values
//! are sampled: each move's mean round points for the seat's team over the
//! worlds drawn from that seat's InfoSet, each world solved exactly, with
//! the standard error over worlds. With W tables (M6-02, D21), each move's
//! mean probability for the seat's team to win the game instead.

use std::fmt::Write as _;

use dominofish_core::infoset::InfoSet;
use dominofish_core::pimc::{self, Limits};
use dominofish_core::record::{GameRecord, move_text};
use dominofish_core::rules::{Phase, team};
use dominofish_core::solver::{Objective, WIN};

use crate::equity::WinTables;
use crate::solve::thousands;

/// The ranking, or an error for a request the record cannot answer.
pub fn run(
    text: &str,
    round: u64,
    ply: u64,
    limits: &Limits,
    seed: u64,
    win: Option<&WinTables>,
) -> Result<String, String> {
    let record =
        GameRecord::from_text(text).map_err(|e| format!("record line {}: {}", e.line, e.reason))?;
    let rounds = record.rounds.len();
    if round == 0 || round > rounds as u64 {
        return Err(format!(
            "--round {round}: the record has rounds 1 to {rounds}"
        ));
    }
    let r = round as usize - 1;
    let moves = record.rounds[r].moves.len();
    if ply > moves as u64 {
        return Err(format!("--ply {ply}: round {round} has {moves} moves"));
    }
    let ply = ply as usize;
    let (mut state, _) = record
        .round_start(r)
        .map_err(|e| format!("round {round}: {e:?}"))?;
    for &mv in &record.rounds[r].moves[..ply] {
        state.play(mv);
    }
    if state.phase != Phase::Playing {
        return Err(format!("round {round} is over after ply {ply}"));
    }
    let seat = state.turn;
    let info = InfoSet::new(&record, r, ply, seat).map_err(|e| format!("{e:?}"))?;
    let objective = match win {
        None => Objective::Points,
        Some(w) => Objective::Win(w.get(record.mode, record.target).ok_or_else(|| {
            format!(
                "no W table for {} target {}",
                record.mode.name(),
                record.target
            )
        })?),
    };
    let a = pimc::analyze_with(&info, limits, objective, seed);
    if a.fallback > 0 || a.inconsistent > 0 {
        return Err(format!(
            "the sampler failed (fallback {}, inconsistent worlds {})",
            a.fallback, a.inconsistent
        ));
    }
    let mode = record.mode;
    let team_label = mode.team_name(team(seat)).to_uppercase();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "DominoFish analyze   mode {}   target {}   round {round}, ply {ply}   seat {} (team {team_label})",
        mode.name(),
        record.target,
        mode.seat_name(seat)
    );
    let _ = writeln!(
        out,
        "PIMC, sampled: {} worlds of {} consistent   seed {seed}   nodes {}   capped solves {}\n",
        a.worlds,
        thousands(u64::try_from(a.info_size).unwrap_or(u64::MAX)),
        thousands(a.nodes),
        a.capped
    );
    // Win values are millionths; shown in percent.
    let (head, scale, sign) = match objective {
        Objective::Points => ("Points", 1.0, "+"),
        Objective::Win(_) => ("Win %", 100.0 / f64::from(WIN), ""),
    };
    let _ = writeln!(out, "{:<20} {:>8} {:>7}", "Move", head, "SE");
    for m in &a.moves {
        let se = m
            .standard_error(a.worlds)
            .map_or("-".to_string(), |s| format!("{:.2}", s * scale));
        let mean = m.mean(a.worlds) * scale;
        let value = if sign == "+" {
            format!("{mean:+.2}")
        } else {
            format!("{mean:.2}")
        };
        let _ = writeln!(out, "{:<20} {value:>8} {se:>7}", move_text(m.mv));
    }
    match objective {
        Objective::Points => {
            let _ = writeln!(
                out,
                "\nPoints: team {team_label}'s round margin from this move on, its own points included, averaged over the sampled worlds."
            );
        }
        Objective::Win(_) => {
            let _ = writeln!(
                out,
                "\nWin %: team {team_label}'s probability to win the game after this move: each world solved for it, with the round's end valued by the equity table W for {} to {}, averaged over the sampled worlds.",
                record.mode.name(),
                record.target
            );
        }
    }
    let _ = writeln!(out, "Best: {}", move_text(a.best));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominofish_core::rules::{Mode, MoveList};

    const LIMITS: Limits = Limits {
        worlds: 8,
        nodes: 10_000_000,
        table_bits: 16,
    };

    /// The move lines of an analysis, in order.
    fn listed(out: &str) -> Vec<String> {
        out.lines()
            .skip_while(|l| !l.starts_with("Move"))
            .skip(1)
            .take_while(|l| !l.is_empty())
            .map(|l| l[..20].trim_end().to_string())
            .collect()
    }

    #[test]
    fn analyze_ranks_every_legal_move_the_same_way_twice() {
        let mut checked = 0;
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            let (_, record) = crate::selfplay::run(mode, 7, 100);
            let text = record.to_text();
            for (r, round) in record.rounds.iter().enumerate() {
                let (mut state, _) = record.round_start(r).unwrap();
                for (ply, &mv) in round.moves.iter().enumerate() {
                    // Positions with at most 18 tiles left keep the debug
                    // build of check.sh fast.
                    if ply % 4 == 0 && dominofish_core::solver::tiles_left(&state) <= 18 {
                        let out = run(&text, r as u64 + 1, ply as u64, &LIMITS, 3, None).unwrap();
                        assert_eq!(
                            out,
                            run(&text, r as u64 + 1, ply as u64, &LIMITS, 3, None).unwrap()
                        );
                        let mut legal = MoveList::new();
                        state.legal_moves(&mut legal);
                        let mut want: Vec<String> =
                            legal.as_slice().iter().map(|&m| move_text(m)).collect();
                        let mut got = listed(&out);
                        assert!(out.contains(&format!("Best: {}", got[0])));
                        got.sort();
                        want.sort();
                        assert_eq!(got, want, "{mode:?} round {} ply {ply}", r + 1);
                        assert!(out.contains("PIMC, sampled: 8 worlds of "));
                        checked += 1;
                    }
                    state.play(mv);
                }
            }
        }
        assert!(checked > 20);
    }

    #[test]
    fn impossible_requests_are_errors() {
        let (_, record) = crate::selfplay::run(Mode::TwoVsTwo, 7, 100);
        let text = record.to_text();
        let n = record.rounds.len() as u64;
        let moves = record.rounds[0].moves.len() as u64;
        assert!(run(&text, 0, 0, &LIMITS, 1, None).is_err(), "round 0");
        assert!(
            run(&text, n + 1, 0, &LIMITS, 1, None).is_err(),
            "a round past the end"
        );
        assert!(
            run(&text, 1, moves + 1, &LIMITS, 1, None).is_err(),
            "a ply past the end"
        );
        let over = run(&text, 1, moves, &LIMITS, 1, None).unwrap_err();
        assert!(over.contains("is over"), "{over}");
        assert!(run("dominofish-record 1\nmode 2v2\n", 1, 0, &LIMITS, 1, None).is_err());
        assert!(run("garbage", 1, 0, &LIMITS, 1, None).is_err());
    }
}

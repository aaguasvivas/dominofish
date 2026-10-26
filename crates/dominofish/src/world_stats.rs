//! `dominofish world-stats` (M4-06): the exact number of consistent worlds
//! (M2-03) at every decision of seeded random games (the selfplay games),
//! summarized by board size, beside the research medians as a reference
//! only (BUILD.md Leads: over 300 random Capi-rule 2v2 games, 7,350 worlds
//! at 13 board tiles and 22 at 20).

use std::fmt::Write as _;

use dominofish_core::infoset::InfoSet;
use dominofish_core::rules::{Mode, Phase};
use dominofish_core::worlds::Worlds;

use crate::solve::thousands;

/// The research medians by board size, for 2v2 only.
fn research(mode: Mode, board: usize) -> &'static str {
    match (mode, board) {
        (Mode::TwoVsTwo, 13) => "7,350",
        (Mode::TwoVsTwo, 20) => "22",
        _ => "-",
    }
}

pub fn run(mode: Mode, games: u64, seed: u64) -> String {
    // World counts by board size (0 to 27 tiles on the board).
    let mut by_board: Vec<Vec<u64>> = vec![Vec::new(); 28];
    let mut decisions = 0u64;
    for g in 0..games {
        let (_, record) = crate::selfplay::run(mode, seed + g, 100);
        for (r, round) in record.rounds.iter().enumerate() {
            let (mut state, _) = record.round_start(r).expect("a selfplay record replays");
            for ply in 0..round.moves.len() {
                if state.phase == Phase::Playing {
                    let info = InfoSet::new(&record, r, ply, state.turn).expect("a decision");
                    let w = Worlds::new(&info).expect("a seat's own decision has no pending draw");
                    by_board[state.board.count_ones() as usize].push(w.count());
                    decisions += 1;
                }
                state.play(round.moves[ply]);
            }
        }
    }
    let mut out = format!(
        "DominoFish world-stats   mode {}   games {games} (selfplay seeds {seed} to {})   decisions {decisions}\n\n",
        mode.name(),
        seed + games - 1
    );
    out.push_str("| Board tiles | Decisions | Median worlds | p10 | p90 | Research median |\n|---|---|---|---|---|---|\n");
    for (board, counts) in by_board.iter_mut().enumerate() {
        if counts.is_empty() {
            continue;
        }
        counts.sort_unstable();
        let at = |q: usize| thousands(counts[(counts.len() - 1) * q / 100]);
        let _ = writeln!(
            out,
            "| {board} | {} | {} | {} | {} | {} |",
            counts.len(),
            at(50),
            at(10),
            at(90),
            research(mode, board)
        );
    }
    out.push_str("\nWorlds: the exact count of consistent worlds of the seat on turn (docs/PLAN.md D11). The research medians (BUILD.md Leads, capi-rules-spec[9]) are a reference only.\n");
    let _ = writeln!(
        out,
        "\nworld-stats: mode {} games {games} decisions {decisions}",
        mode.name()
    );
    out
}

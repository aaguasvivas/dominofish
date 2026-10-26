//! `dominofish selfplay`: one game of uniformly random canonical legal moves,
//! printed as a readable log: each hand as dealt, every move with its tile,
//! every scoring event with the scores after it, and the line of play at the
//! end of each round. Round r's deal uses the stream derive(seed, r); moves
//! use derive(seed, 0).

use std::collections::VecDeque;
use std::fmt::Write as _;

use dominofish_core::rng::{Rng, derive};
use dominofish_core::rules::{Deal, End, Event, Mode, Move, MoveList, Phase, State, team};
use dominofish_core::tile::Tile;

/// The column where the scores start on a line with a scoring event.
const SCORE_COLUMN: usize = 62;
/// Tiles per line in a hand or board listing.
const TILES_PER_LINE: usize = 10;

fn tile(t: Tile) -> String {
    format!("[{}|{}]", t.low(), t.high())
}

fn team_label(mode: Mode, t: usize) -> String {
    mode.team_name(t).to_uppercase()
}

fn scores(mode: Mode, state: &State) -> String {
    format!(
        "{} {:>3}   {} {:>3}",
        team_label(mode, 0),
        state.scores[0],
        team_label(mode, 1),
        state.scores[1]
    )
}

/// Tiles in rows of TILES_PER_LINE, the first row after `label`, the rest
/// aligned under it.
fn rows(out: &mut String, label: &str, tiles: &[String], sep: &str) {
    let indent = " ".repeat(label.len());
    for (i, chunk) in tiles.chunks(TILES_PER_LINE).enumerate() {
        let head = if i == 0 { label } else { &indent };
        let _ = writeln!(out, "{head}{}", chunk.join(sep));
    }
}

fn event_text(mode: Mode, event: &Event) -> String {
    let seat = |s: u8| mode.seat_name(s);
    match *event {
        Event::PaseSalida { team } => format!("pase de salida, {} +25", team_label(mode, team)),
        Event::PaseCorrido { team } => format!("pase corrido, {} +25", team_label(mode, team)),
        Event::Domino { seat: s, points } => {
            format!(
                "domino by {}, {} +{points}",
                seat(s),
                team_label(mode, team(s))
            )
        }
        Event::Capicua { seat: s, points } => {
            format!(
                "capicua by {}, {} +{points}",
                seat(s),
                team_label(mode, team(s))
            )
        }
        Event::Tranque {
            blocker,
            rival,
            blocker_pips,
            rival_pips,
            winner,
            points,
        } => format!(
            "tranque, {} {blocker_pips} pips, {} {rival_pips} pips, {} +{points}",
            seat(blocker),
            seat(rival),
            team_label(mode, team(winner))
        ),
    }
}

fn write_round(out: &mut String, mode: Mode, round: u64, deal: &Deal, leader: Option<u8>) {
    let _ = write!(out, "\nRound {round}");
    if let Some(s) = leader {
        let _ = write!(out, "   {} leads", mode.seat_name(s));
    }
    out.push('\n');
    for seat in 0..mode.seats() {
        let label = if seat == 0 {
            "  Hands     "
        } else {
            "            "
        };
        let hand: Vec<String> = deal.hand(seat).iter().map(|&t| tile(t)).collect();
        rows(
            out,
            &format!("{label}{}  ", mode.seat_name(seat)),
            &hand,
            " ",
        );
    }
    let pile: Vec<String> = deal.boneyard(mode).iter().map(|&t| tile(t)).collect();
    if !pile.is_empty() {
        rows(out, "  Boneyard     ", &pile, " ");
    }
}

/// The line of play, left to right, each tile oriented as it lies.
struct Line(VecDeque<(u8, u8)>);

impl Line {
    fn start(state: &State) -> Line {
        let mut line = Line(VecDeque::new());
        if state.board != 0 {
            line.0.push_back((state.left, state.right));
        }
        line
    }

    /// Records a play from the state after it.
    fn played(&mut self, t: Tile, end: End, was_empty: bool, after: &State) {
        if was_empty {
            self.0.push_back((after.left, after.right));
        } else if end == End::Left {
            self.0.push_front((after.left, t.other(after.left)));
        } else {
            self.0.push_back((t.other(after.right), after.right));
        }
    }

    fn write(&self, out: &mut String) {
        let tiles: Vec<String> = self.0.iter().map(|(a, b)| format!("[{a}|{b}]")).collect();
        rows(out, "  Board     ", &tiles, "");
    }
}

pub fn run(mode: Mode, seed: u64, target: u16) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "DominoFish selfplay   mode {}   seed {seed}   target {target}",
        mode.name()
    );
    let mut moves_rng = Rng::new(derive(seed, 0));
    let mut round = 1u64;
    let deal = Deal::shuffled(&mut Rng::new(derive(seed, round)));
    write_round(&mut out, mode, round, &deal, None);
    let (mut state, opening) = State::first_round(mode, target, &deal);
    let _ = writeln!(
        out,
        "  {:<3} opens   {}",
        mode.seat_name(state.starter),
        tile(opening)
    );
    let mut line = Line::start(&state);
    let mut moves = MoveList::new();
    let mut count = 0u32;
    loop {
        match state.phase {
            Phase::Finished => break,
            Phase::RoundOver => {
                round += 1;
                let deal = Deal::shuffled(&mut Rng::new(derive(seed, round)));
                state = state.next_round(&deal);
                write_round(&mut out, mode, round, &deal, Some(state.starter));
                line = Line::start(&state);
            }
            Phase::Playing => {
                state.legal_moves(&mut moves);
                let mv = moves.as_slice()[moves_rng.below(moves.len() as u32) as usize];
                let seat = state.turn;
                let pile_before = state.boneyard().to_vec();
                let was_empty = state.board == 0;
                let event = state.play(mv).event;
                count += 1;
                let mut text = format!("  {:<3} ", mode.seat_name(seat));
                match mv {
                    Move::Play { tile: t, end } => {
                        line.played(t, end, was_empty, &state);
                        let side = if was_empty {
                            "lead"
                        } else if end == End::Left {
                            "left"
                        } else {
                            "right"
                        };
                        let _ = write!(text, "plays   {} {side}", tile(t));
                    }
                    Move::Draw => {
                        let drawn: Vec<String> = pile_before[state.boneyard().len()..]
                            .iter()
                            .rev()
                            .map(|&t| tile(t))
                            .collect();
                        let _ = write!(text, "draws   {}", drawn.join(" "));
                    }
                    Move::Pass => text.push_str("passes"),
                }
                if let Some(e) = event {
                    let _ = write!(text, "   {}", event_text(mode, &e));
                    let pad = SCORE_COLUMN.saturating_sub(text.len()).max(3);
                    let _ = write!(text, "{}{}", " ".repeat(pad), scores(mode, &state));
                }
                out.push_str(text.trim_end());
                out.push('\n');
                if state.phase != Phase::Playing {
                    line.write(&mut out);
                }
            }
        }
    }
    let winner = if state.scores[0] >= target { 0 } else { 1 };
    let _ = writeln!(
        out,
        "\nFinal   {}   {} wins in {round} rounds, {count} moves",
        scores(mode, &state),
        team_label(mode, winner)
    );
    out
}

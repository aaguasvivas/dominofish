//! State text (docs/FORMATS.md): one line that holds a full state, every
//! hand and the boneyard order included. It carries fixed solver positions
//! (the bench set) and the omniscient bot's view; nothing that sees only an
//! InfoSet ever reads it.

use crate::record::{parse_seat, parse_tile};
use crate::rules::{MAX_TARGET, Mode, NO_SEAT, Parts, Phase, State};
use crate::tile::{ALL, TILE_COUNT, Tile, tiles};

fn tile_list(mask: u32) -> String {
    if mask == 0 {
        return "-".into();
    }
    tiles(mask)
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_mask(s: &str) -> Option<u32> {
    if s == "-" {
        return Some(0);
    }
    let mut mask = 0u32;
    for t in s.split(',') {
        let bit = parse_tile(t)?.bit();
        if mask & bit != 0 {
            return None;
        }
        mask |= bit;
    }
    Some(mask)
}

/// The state as one line of state text.
pub fn to_text(state: &State) -> String {
    let mode = state.mode;
    let seat = |s: u8| if s == NO_SEAT { "-" } else { mode.seat_name(s) };
    let hands: Vec<String> = (0..mode.seats())
        .map(|s| tile_list(state.hands[s as usize]))
        .collect();
    let pile: Vec<String> = state.boneyard().iter().map(Tile::to_string).collect();
    format!(
        "v=1 mode={} target={} scores={},{} hands={} board={:x} ends={} turn={} starter={} last={} passes={} phase={} winner={} pile={}",
        if mode == Mode::OneVsOne { "1v1" } else { "2v2" },
        state.target,
        state.scores[0],
        state.scores[1],
        hands.join("/"),
        state.board,
        if state.board == 0 {
            "--".to_string()
        } else {
            format!("{}{}", state.left, state.right)
        },
        seat(state.turn),
        seat(state.starter),
        seat(state.last_played_by),
        state.passes,
        match state.phase {
            Phase::Playing => "P",
            Phase::RoundOver => "R",
            Phase::Finished => "F",
        },
        seat(state.round_winner),
        if pile.is_empty() {
            "-".to_string()
        } else {
            pile.join(",")
        },
    )
}

/// Parses one line of state text. Rejects any field out of range and any
/// set of tiles that is not a partition of the 28 tiles into the hands, the
/// board, and the boneyard.
pub fn from_text(line: &str) -> Option<State> {
    const KEYS: [&str; 13] = [
        "v", "mode", "target", "scores", "hands", "board", "ends", "turn", "starter", "last",
        "passes", "phase", "winner",
    ];
    let words: Vec<&str> = line.split(' ').collect();
    if words.len() != KEYS.len() + 1 {
        return None;
    }
    let mut v = Vec::with_capacity(words.len());
    for (word, key) in words.iter().zip(KEYS.iter().chain(["pile"].iter())) {
        v.push(word.strip_prefix(key)?.strip_prefix('=')?);
    }
    if v[0] != "1" {
        return None;
    }
    let mode = match v[1] {
        "1v1" => Mode::OneVsOne,
        "2v2" => Mode::TwoVsTwo,
        _ => return None,
    };
    let target: u16 = v[2].parse().ok()?;
    let (s0, s1) = v[3].split_once(',')?;
    let scores = [s0.parse().ok()?, s1.parse().ok()?];
    if target == 0 || target > MAX_TARGET || scores.iter().any(|&s: &u16| s >= 512) {
        return None;
    }
    let groups: Vec<&str> = v[4].split('/').collect();
    if groups.len() != mode.seats() as usize {
        return None;
    }
    let mut hands = [0u32; 4];
    for (h, g) in hands.iter_mut().zip(&groups) {
        *h = parse_mask(g)?;
    }
    let board = u32::from_str_radix(v[5], 16).ok()?;
    let (left, right) = if v[6] == "--" {
        (0, 0)
    } else {
        let e = v[6].as_bytes();
        if e.len() != 2 || !(b'0'..=b'6').contains(&e[0]) || !(b'0'..=b'6').contains(&e[1]) {
            return None;
        }
        (e[0] - b'0', e[1] - b'0')
    };
    if (board == 0) != (v[6] == "--") {
        return None;
    }
    let seat = |s: &str| {
        if s == "-" {
            Some(NO_SEAT)
        } else {
            parse_seat(mode, s)
        }
    };
    let (turn, starter, last, winner) = (seat(v[7])?, seat(v[8])?, seat(v[9])?, seat(v[12])?);
    if turn == NO_SEAT || starter == NO_SEAT {
        return None;
    }
    let passes: u8 = v[10].parse().ok()?;
    let phase = match v[11] {
        "P" => Phase::Playing,
        "R" => Phase::RoundOver,
        "F" => Phase::Finished,
        _ => return None,
    };
    let pile: Vec<Tile> = if v[13] == "-" {
        vec![]
    } else {
        v[13].split(',').map(parse_tile).collect::<Option<_>>()?
    };
    let pile_mask = pile.iter().fold(0u32, |m, t| m | t.bit());
    let max_pile = if mode == Mode::OneVsOne { 14 } else { 0 };
    let held = hands.iter().fold(0u32, |m, h| m | h);
    let count =
        hands.iter().map(|h| h.count_ones()).sum::<u32>() + board.count_ones() + pile.len() as u32;
    if passes > 3
        || pile.len() > max_pile
        || count != TILE_COUNT as u32
        || held | board | pile_mask != ALL
    {
        return None;
    }
    let parts = Parts {
        mode,
        target,
        scores,
        hands,
        board,
        left,
        right,
        turn,
        starter,
        last_played_by: last,
        passes,
        phase,
        round_winner: winner,
    };
    Some(State::from_parts(&parts, &pile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::rules::{Deal, MoveList};

    #[test]
    fn state_text_round_trips_over_seeded_playouts() {
        let mut moves = MoveList::new();
        let mut checked = 0;
        for (mode, seed) in [(Mode::TwoVsTwo, 1u64), (Mode::OneVsOne, 2)] {
            let mut rng = Rng::new(seed);
            for _ in 0..400 {
                let deal = Deal::shuffled(&mut rng);
                let (mut state, _) = State::first_round(mode, 100, &deal);
                loop {
                    let text = to_text(&state);
                    // Slots past the boneyard's end keep drawn tiles for
                    // undo, so the check compares what the state shows.
                    let back = from_text(&text).expect("parses");
                    assert_eq!(to_text(&back), text);
                    assert_eq!(back.hash(), state.hash(), "{text}");
                    assert_eq!(back.position_hash(), state.position_hash());
                    let mut theirs = MoveList::new();
                    state.legal_moves(&mut moves);
                    back.legal_moves(&mut theirs);
                    assert_eq!(moves.as_slice(), theirs.as_slice(), "{text}");
                    checked += 1;
                    if state.phase != Phase::Playing {
                        break;
                    }
                    state.legal_moves(&mut moves);
                    let mv = moves.as_slice()[rng.below(moves.len() as u32) as usize];
                    state.play(mv);
                }
            }
        }
        assert!(checked > 10_000, "{checked}");
    }

    #[test]
    fn malformed_state_text_is_rejected() {
        let deal = Deal::shuffled(&mut Rng::new(3));
        let (state, _) = State::first_round(Mode::OneVsOne, 100, &deal);
        let good = to_text(&state);
        assert!(from_text(&good).is_some());
        let first_pile_tile = state.boneyard()[0].to_string();
        let bad = [
            good.replace("v=1", "v=2"),
            good.replace("mode=1v1", "mode=3v3"),
            good.replace("target=100", "target=301"),
            good.replace("turn=", "turn=x"),
            good.replace(&format!("pile={first_pile_tile},"), "pile="),
            good.replace("passes=0", "passes=4"),
            good.replace(" phase=P", " phase=Q"),
            format!("{good} extra=1"),
            good.replacen(' ', "  ", 1),
        ];
        for b in bad {
            assert!(from_text(&b).is_none(), "accepted: {b}");
        }
    }
}

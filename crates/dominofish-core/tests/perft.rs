//! M1-06: perft counts from fixtures/perft.txt (generated from Capi's engine
//! by tools/capi-oracle/perft.mjs), matched exactly with divide per root
//! move. Format: docs/FORMATS.md, Perft.

mod common;

use dominofish_core::perft;
use dominofish_core::rules::{Deal, End, Mode, Move, State};
use dominofish_core::tile::Tile;

fn move_text(mv: Move) -> String {
    match mv {
        Move::Play { tile, end } => format!("{tile}{}", if end == End::Left { "L" } else { "R" }),
        Move::Draw => "D".into(),
        Move::Pass => "P".into(),
    }
}

fn parse_move(s: &str) -> Move {
    match s {
        "D" => Move::Draw,
        "P" => Move::Pass,
        _ => {
            let (tile, end) = s.split_at(s.len() - 1);
            let (a, b) = tile.split_once('-').expect("a-b");
            Move::Play {
                tile: Tile::new(a.parse().unwrap(), b.parse().unwrap()),
                end: if end == "L" { End::Left } else { End::Right },
            }
        }
    }
}

#[test]
fn perft_matches_capi_with_divide() {
    let text = std::fs::read_to_string(common::fixture("perft.txt")).expect("fixtures/perft.txt");
    let (mut queries, mut mismatches, mut total_lines) = (0, 0, 0u64);
    for line in text.lines().filter(|l| l.starts_with("Q ")) {
        let w: Vec<&str> = line.split(' ').collect();
        let field = |k: &str| w.iter().find_map(|x| x.strip_prefix(k)).expect(k);
        let mode = if w[2] == "1v1" {
            Mode::OneVsOne
        } else {
            Mode::TwoVsTwo
        };
        let target: u16 = w[3].parse().unwrap();
        let mut tiles = [Tile::from_index(0); 28];
        for (k, v) in field("deal=").split(',').enumerate() {
            tiles[k] = Tile::from_index(v.parse().unwrap());
        }
        let deal = Deal::from_tiles(tiles).expect("deal");
        let mut state = if w[4] == "round1" {
            State::first_round(mode, target, &deal).0
        } else {
            let p: Vec<&str> = w[4].split(':').collect();
            let leader = (0..mode.seats())
                .find(|&s| mode.seat_name(s) == p[1])
                .unwrap();
            State::dealt(
                mode,
                target,
                [p[2].parse().unwrap(), p[3].parse().unwrap()],
                &deal,
                leader,
            )
        };
        let path = field("path=");
        if path != "-" {
            for m in path.split(',') {
                let mv = parse_move(m);
                assert!(state.is_legal(mv), "{}: path move {m} is illegal", w[1]);
                state.play(mv);
            }
        }
        let depth = match field("depth=") {
            "all" => u32::MAX,
            d => d.parse().unwrap(),
        };
        let want: u64 = field("total=").parse().unwrap();
        let got = perft::lines(&mut state, depth);
        let mut out = [(Move::Pass, 0); 16];
        let n = perft::divide(&mut state, depth, &mut out);
        let mut got_divide: Vec<String> = out[..n]
            .iter()
            .map(|(m, c)| format!("{}:{c}", move_text(*m)))
            .collect();
        let mut want_divide: Vec<String> = match field("divide=") {
            "-" => vec![],
            d => d.split(',').map(String::from).collect(),
        };
        got_divide.sort();
        want_divide.sort();
        if got != want || got_divide != want_divide {
            mismatches += 1;
            println!(
                "ERROR perft {}: total want {want} got {got}; divide want {want_divide:?} got {got_divide:?}",
                w[1]
            );
        }
        queries += 1;
        total_lines += got;
    }
    println!("perft: queries {queries} lines {total_lines} mismatches {mismatches}");
    assert!(queries > 0);
    assert_eq!(mismatches, 0);
}

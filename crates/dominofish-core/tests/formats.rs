//! M2-02: the game record text format and the engine line protocol
//! (docs/FORMATS.md) round-trip: print then parse gives the same value, and
//! parse then print gives the same text, over seeded random games in both
//! modes at targets 100 and 200; malformed input is rejected with its line.

mod common;

use dominofish_core::infoset::Action;
use dominofish_core::protocol::{FromEngine, ToEngine, View, messages_for};
use dominofish_core::record::GameRecord;
use dominofish_core::rules::{End, Mode, Move};
use dominofish_core::tile::Tile;

fn records() -> Vec<GameRecord> {
    let mut out = Vec::new();
    for seed in 0..100 {
        for mode in [Mode::OneVsOne, Mode::TwoVsTwo] {
            for target in [100, 200] {
                out.push(common::random_record(mode, target, seed, true));
            }
        }
    }
    out
}

#[test]
fn records_round_trip() {
    let (mut games, mut lines) = (0, 0);
    for record in records() {
        let text = record.to_text();
        let parsed = GameRecord::from_text(&text).expect("printed records parse");
        assert_eq!(parsed, record);
        assert_eq!(parsed.to_text(), text);
        games += 1;
        lines += text.lines().count();
    }
    println!("formats records: games {games} lines {lines} round-trip exactly");
}

#[test]
fn malformed_records_are_rejected_with_their_line() {
    let good = common::random_record(Mode::TwoVsTwo, 100, 7, false).to_text();
    let lines: Vec<&str> = good.lines().collect();
    let with = |n: usize, new: &str| {
        let mut l = lines.clone();
        l[n] = new;
        l.join("\n")
    };
    // Line 5 is the first move (after the header and the first deal).
    let cases = [
        (with(0, "dominofish-record 2"), 1),
        (with(1, "mode 3v3"), 2),
        (with(2, "target 0"), 3),
        (with(3, "deal 0-0 0-1"), 4),
        (with(4, "x pass"), 5),
        (with(4, &format!("{} jump", &lines[4][..1])), 5),
    ];
    for (text, line) in &cases {
        let e = GameRecord::from_text(text).expect_err("malformed");
        assert_eq!(e.line, *line, "{e:?}");
    }
    // A move by the wrong seat, and an illegal move, keep their line too.
    let seat = &lines[4][..1];
    let other = if seat == "n" { "e" } else { "n" };
    let e = GameRecord::from_text(&with(4, &lines[4].replacen(seat, other, 1))).unwrap_err();
    assert_eq!(e.line, 5, "{e:?}");
    let e = GameRecord::from_text(&with(4, &format!("{seat} draw"))).unwrap_err();
    assert_eq!(e.line, 5, "{e:?}");
    assert!(GameRecord::from_text(&good.replace("end\n", "")).is_err());
    println!(
        "formats malformed: {} rejected at their line",
        cases.len() + 3
    );
}

#[test]
fn protocol_round_trips() {
    let mut messages = 0;
    for record in records() {
        for seat in 0..record.mode.seats() {
            for msg in messages_for(&record, seat, "white=abc123 black=def456 seeds=1000", 42) {
                let line = msg.to_line(record.mode);
                assert_eq!(
                    ToEngine::parse(&line, record.mode),
                    Some(msg.clone()),
                    "{line}"
                );
                assert_eq!(
                    ToEngine::parse(&line, record.mode)
                        .unwrap()
                        .to_line(record.mode),
                    line
                );
                messages += 1;
            }
        }
        for round in &record.rounds {
            for &mv in &round.moves {
                let msg = FromEngine::Move(mv);
                assert_eq!(FromEngine::parse(&msg.to_line()), Some(msg));
            }
        }
    }
    // The omniscient bot's state message round-trips in both modes.
    for (mode, seed) in [(Mode::OneVsOne, 3u64), (Mode::TwoVsTwo, 4)] {
        use dominofish_core::rng::Rng;
        use dominofish_core::rules::{Deal, State};
        let (state, _) = State::first_round(mode, 100, &Deal::shuffled(&mut Rng::new(seed)));
        let msg = ToEngine::State(Box::new(state));
        assert_eq!(ToEngine::parse(&msg.to_line(mode), mode), Some(msg));
    }
    for msg in [
        FromEngine::Ready {
            version: 1,
            budgets: "-".into(),
            threads: 1,
            tables: "-".into(),
            view: View::InfoSet,
        },
        FromEngine::Ready {
            version: 1,
            budgets: "nodes:10000000".into(),
            threads: 1,
            tables: "16MB".into(),
            view: View::Omniscient,
        },
        FromEngine::Header("a=1 b=2".into()),
        FromEngine::Info("nodes 123".into()),
    ] {
        assert_eq!(FromEngine::parse(&msg.to_line()), Some(msg));
    }
    // Exact lines, as docs/FORMATS.md writes them.
    let two = Mode::TwoVsTwo;
    assert_eq!(
        ToEngine::Opening {
            seat: 1,
            tile: Tile::new(6, 6)
        }
        .to_line(two),
        "opening e 6-6"
    );
    assert_eq!(
        ToEngine::Played {
            seat: 2,
            action: Action::Play {
                tile: Tile::new(3, 6),
                end: End::Left
            }
        }
        .to_line(two),
        "played s play 3-6 left"
    );
    assert_eq!(
        ToEngine::Played {
            seat: 1,
            action: Action::Draw { count: 3 }
        }
        .to_line(Mode::OneVsOne),
        "played s draw 3"
    );
    assert_eq!(FromEngine::Move(Move::Pass).to_line(), "move pass");
    for bad in [
        "",
        "go now",
        "played x pass",
        "round 1 scores=1 leader=n hand=0-0",
        "drew 7-1",
        "game mode=2v2 target=100 seat=q seed=1",
    ] {
        assert_eq!(ToEngine::parse(bad, two), None, "{bad}");
    }
    for bad in ["move", "move play 9-9 left", "ready", "move jump"] {
        assert_eq!(FromEngine::parse(bad), None, "{bad}");
    }
    println!("formats protocol: messages {messages} round-trip exactly; malformed lines rejected");
}

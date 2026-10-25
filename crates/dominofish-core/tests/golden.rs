//! M1-04: replays the golden traces in fixtures/golden (generated from Capi's
//! engine by tools/capi-oracle/traces.mjs) through the referee and compares
//! the opener, every event, and the canonical state after every move. Needs
//! no Capi checkout.

mod common;

use std::path::PathBuf;

#[test]
fn golden_traces_replay_with_zero_mismatches() {
    let dir = common::fixture("golden");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("fixtures/golden exists")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt") && !p.ends_with("coverage.txt"))
        .collect();
    paths.sort();
    let (mut games, mut moves, mut mismatches) = (0, 0, 0);
    for p in &paths {
        let report = common::replay(p);
        for e in report.errors.iter().take(10) {
            println!("{e}");
        }
        games += report.games.len();
        moves += report.moves;
        mismatches += report.mismatches;
    }
    println!(
        "golden: files {} games {games} moves {moves} mismatches {mismatches}",
        paths.len()
    );
    assert!(paths.len() >= 6 && games > 0);
    assert_eq!(mismatches, 0);
}

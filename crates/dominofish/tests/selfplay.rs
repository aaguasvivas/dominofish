//! M0: selfplay finishes a full game in both modes, and two runs with one
//! seed print identical output.

use std::process::Command;

fn selfplay(mode: &str, seed: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_dominofish"))
        .args(["selfplay", "--mode", mode, "--seed", seed])
        .output()
        .expect("run dominofish");
    assert!(
        out.status.success(),
        "selfplay {mode} exited {}",
        out.status
    );
    String::from_utf8(out.stdout).expect("utf-8 output")
}

#[test]
fn selfplay_is_deterministic_and_finishes_in_both_modes() {
    for mode in ["2v2", "1v1"] {
        let first = selfplay(mode, "1");
        assert_eq!(first, selfplay(mode, "1"), "two runs of {mode} differ");
        let last = first.lines().last().expect("output");
        assert!(last.starts_with("final "), "{mode}: {last}");
        // "final <team> <score> <team> <score> winner ..."
        let fields: Vec<&str> = last.split(' ').collect();
        let scores = [fields[2], fields[4]].map(|v| v.parse::<u16>().expect("score"));
        assert!(scores.iter().any(|&s| s >= 100), "{mode}: {last}");
    }
}

//! M6-04: `match --threads` gives the same result file and summary at any
//! thread count: for a fixed number of pairs, for an SPRT that stops the
//! match, for the count of searches that reached their cap, and with fewer
//! pairs than threads. The harness refuses binaries under target/, so the
//! test copies the built binary to a temporary directory.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The built binary, copied outside target/.
fn binary() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dominofish-match-threads-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the temporary directory");
    let bin = dir.join("dominofish");
    std::fs::copy(env!("CARGO_BIN_EXE_dominofish"), &bin).expect("copy the binary");
    bin
}

/// The result file with the `# time` line cut to its move counts (its
/// reply times are wall time), and the summary line printed, without the
/// results path.
fn result(bin: &Path, label: &str, threads: u32, args: &[&str], a: &str, b: &str) -> String {
    let out = bin.with_file_name(format!("{label}-{threads}.txt"));
    let run = Command::new(bin)
        .arg("match")
        .args(args)
        .args(["--threads", &threads.to_string()])
        .args(["--a", &format!("{} engine {a}", bin.display())])
        .args(["--b", &format!("{} engine {b}", bin.display())])
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run dominofish match");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{label} on {threads} threads: {stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(&out).expect("read the result file");
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| match l.strip_prefix("# time ") {
            // "a=<mean>,<max>,<moves> b=<mean>,<max>,<moves>"
            Some(t) => t
                .split(' ')
                .map(|side| match side.split_once('=') {
                    Some((name, v)) => format!("{name}={}", v.rsplit(',').next().unwrap_or("")),
                    None => side.to_string(),
                })
                .fold("# time".to_string(), |s, x| s + " " + &x),
            None => l.to_string(),
        })
        .collect();
    let summary = stdout.lines().last().unwrap_or("");
    lines.push(summary.split("; results ").next().unwrap_or("").to_string());
    lines.join("\n")
}

#[test]
fn match_files_are_the_same_at_any_thread_count() {
    let bin = binary();
    let (rule, random) = ("--bot rule", "--bot random");
    let games: &[&str] = &[
        "--mode",
        "1v1",
        "--unit",
        "game",
        "--target",
        "100",
        "--pairs",
        "12",
        "--seed",
        "990200000",
        "--engine-seed",
        "5000000992",
    ];
    // The SPRT stops the match at its decision, so the pairs other threads
    // started past it must be discarded, their moves and searches too.
    let sprt: &[&str] = &[
        "--mode",
        "2v2",
        "--unit",
        "round",
        "--target",
        "100",
        "--pairs",
        "1000",
        "--seed",
        "990300000",
        "--engine-seed",
        "5000000993",
        "--sprt",
        "0,4",
    ];
    let capped: &[&str] = &[
        "--mode",
        "2v2",
        "--unit",
        "round",
        "--target",
        "100",
        "--pairs",
        "300",
        "--seed",
        "990500000",
        "--engine-seed",
        "5000000995",
        "--sprt",
        "0,4",
    ];
    let one_pair: &[&str] = &[
        "--mode",
        "1v1",
        "--unit",
        "round",
        "--target",
        "100",
        "--pairs",
        "1",
        "--seed",
        "990600000",
        "--engine-seed",
        "5000000996",
    ];
    // AIVAT values and their counts: draw terms and both bots' action terms.
    let aivat: &[&str] = &[
        "--mode",
        "1v1",
        "--unit",
        "round",
        "--target",
        "100",
        "--pairs",
        "4",
        "--seed",
        "990800000",
        "--engine-seed",
        "5000000998",
        "--aivat",
        "2",
    ];
    let cases = [
        ("games", games, rule, random),
        ("sprt", sprt, rule, random),
        // The omniscient bot with a small node cap reports capped searches.
        ("capped", capped, "--bot omniscient --nodes 2000", random),
        ("one-pair", one_pair, rule, random),
        ("aivat", aivat, rule, random),
    ];
    for (label, args, a, b) in cases {
        let one = result(&bin, label, 1, args, a, b);
        let pairs = one.lines().filter(|l| l.starts_with("pair ")).count();
        match label {
            "games" => assert_eq!(pairs, 12, "{one}"),
            "one-pair" => assert_eq!(pairs, 1, "{one}"),
            "aivat" => {
                assert_eq!(pairs, 4, "{one}");
                assert!(
                    one.lines()
                        .any(|l| l.starts_with("# aivat j=2 ") && !l.contains(" values=0 ")),
                    "no AIVAT values to count: {one}"
                );
            }
            _ => assert!(
                pairs < 300,
                "{label}: the SPRT did not stop the match: {one}"
            ),
        }
        if label == "capped" {
            assert!(
                one.lines()
                    .any(|l| l.starts_with("# capped a=") && !l.starts_with("# capped a=0 ")),
                "no capped searches to count: {one}"
            );
        }
        for threads in [2, 4] {
            assert_eq!(
                one,
                result(&bin, label, threads, args, a, b),
                "{label}: 1 and {threads} threads differ"
            );
        }
    }
    let _ = std::fs::remove_dir_all(bin.parent().expect("the temporary directory"));
}

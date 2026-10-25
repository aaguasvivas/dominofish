//! M1-07 under cargo test: a short lockstep differential fuzz against Capi's
//! engine in both modes. Runs when CAPI_DIR (default
//! /Users/Adelson/Desktop/personal/capi) holds the pinned commit; otherwise it
//! prints SKIP and passes. The features.json verify command runs the full
//! 1,000,000 rounds per mode and fails on SKIP.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn capi_fuzz_short_run_in_both_modes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for mode in ["1v1", "2v2"] {
        let out = Command::new(env!("CARGO_BIN_EXE_dominofish"))
            .args([
                "capi-fuzz",
                "--mode",
                mode,
                "--rounds",
                "2000",
                "--seed",
                "1",
            ])
            .current_dir(&root)
            .output()
            .expect("run dominofish capi-fuzz");
        let text = String::from_utf8_lossy(&out.stdout);
        let summary = text.lines().last().unwrap_or("");
        println!("{text}");
        if summary.starts_with("capi-fuzz: SKIP") {
            continue;
        }
        assert!(
            out.status.success(),
            "{mode}: {text}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(summary.contains("mismatches 0"), "{summary}");
    }
}

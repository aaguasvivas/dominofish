# DominoFish progress

## Top block

- Milestone: M1 (rules parity); M0 passed 2026-10-25
- Feature in progress: M1-05
- Last green commit: 2466844 (M0-99)
- Last Bench: none (Bench starts at M3)
- Running jobs: none
- Open questions: Q1 to Q4 in docs/PLAN.md, Decision log (sent 2026-10-25, unanswered)
- Stashes: none

## 2026-10-25

- Installed Rust with Homebrew's rustup 1.29.1 (keg-only); added export PATH="/opt/homebrew/opt/rustup/bin:$PATH" to ~/.zshrc. Pinned rustc 1.99.1 with clippy, rustfmt, and wasm32-unknown-unknown; uninstalled the duplicate stable toolchain.
- Created the repository, the Cargo workspace (dominofish-core, dominofish), the state files, scripts/check.sh, scripts/regress.sh, and the M0 verify scripts.
- dominofish selfplay plays random canonical legal moves to a finished game in both modes. The rules in core/src/rules.rs are written from the digest's rule list and a reading of the pinned Capi code; no rules fidelity is claimed until M1.
- Committed 2466844 (M0-99). Then every M0 verify exited 0 in this session and the M0 features were marked passing:
  - M0-01: PASS rustc 1.99.1 (58cf8f9c3 2026-10-13), clippy, rustfmt, wasm32 target, jobs 4, line-tables-only
  - M0-02: PASS check.sh exit 0 (check: PASS in 1s); exit 1 with an em dash present
  - M0-03: PASS seed 1: 2v2 109-69 in 3 rounds, 68 moves; 1v1 69-116 in 9 rounds, 203 moves; identical on two runs; cargo test selfplay ok
  - M0-04: PASS CLAUDE.md 26 lines; PLAN.md headings; features.json 35 entries, M0 to M11; progress.md top block; regress.sh selection
  - M0-99: PASS M0-01 ok; M0-02 ok; M0-03 ok; M0-04 ok; on main with 1 commits
  - regress: PASS 5 of 5 fast commands in 5s
- No strength numbers yet.
- M1-01 passed: docs/RULES.md (R1 to R28, L1 to L9, moves, intent map, canonical state, 36 rule paths). Verify: M1-01: PASS R1-R28 and L1-L9 present; 76 cites, 76 anchors found in range at 1d8868a; 36 rule paths (14 from Capi's fuzz). Negative control: moving one cite to the wrong line made it exit 1. Added scripts/mark-pass.sh, which sets passes only after the verify command exits 0.
- M1-02 passed: play and undo with an Undo record, a Zobrist hash (tile places incremental, scalars per call), State::is_legal for raw Capi moves, State::canonical, Deal::from_tiles, and core/src/referee.rs. Verify: M1-02: PASS play, undo, and hash exact over 2000 seeded games (target 2000, 1000 per mode), 262729 moves; referee rejects illegal moves; Capi-ordered deals. Mutation check: dropping the passes restore in undo, or one hash update in draw, makes the test fail. Selfplay output for seed 1 is unchanged.
- M1-03 passed: tools/capi-oracle/build.sh, lib.mjs (mulberry32 copy, fixedDealRng, dealFixedRound), selftest.mjs, docs/FORMATS.md. Verify: M1-03: PASS built tools/capi-oracle/build/capi-engine.mjs from 1d8868a26ce930f478f98e64060c06b1d1a2c1f4 (23320 bytes); refuses a CAPI_DIR without the pin; CAPI_DIR unchanged; selftest: PASS bundle exports ok; fixedDealRng reproduced 1000 of 1000 deals (both modes); whole-suit seeds 4 of 4.
- M1-04 passed: tools/capi-oracle/traces.mjs writes fixtures/golden (608 games, 117,456 moves, 9.2 MB); crates/dominofish-core/tests/golden.rs replays them. Verify: M1-04: PASS golden: files 6 games 608 moves 117456 mismatches 0 (target 0); fixtures 9200 KB (target under 20480); identical to a fresh traces.mjs run at 1d8868a; Capi's fuzz paths and left leads reached, fewest 5 (2v2 tranque-tie-opener-not-compared).
- Negative controls on the replay: tranque ties to the blocker instead of the opener gives mismatches (first at 1v1-100.txt:8431, seed 100047). Paying a bonus at score + 25 == target (<= instead of <) gives 0 mismatches: no trace holds a bonus exactly 25 below the target (coverage: bonus-25-short absent, bonus-26-short 2). M1-05 (the cap edge tests) and M1-07 (the bonus-25-short counter) must cover it. 1v1 open-no-double is also absent from the traces (about 0.3% of 1v1 deals); M1-05 and M1-07 cover it.
- Next: M1-05 (edge cases and the published deals).

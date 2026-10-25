# DominoFish progress

## Top block

- Milestone: M1 (rules parity); M0 passed 2026-10-25
- Feature in progress: M1-02
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
- Next: M1-02 (undo, hash, Capi-ordered deals).

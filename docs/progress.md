# DominoFish progress

## Top block

- Milestone: M0 (bootstrap)
- Feature in progress: M0-99
- Last green commit: none yet
- Last Bench: none (Bench starts at M3)
- Running jobs: none
- Open questions: Q1 to Q4 in docs/PLAN.md, Decision log (sent 2026-10-25, unanswered)
- Stashes: none

## 2026-10-25

- Installed Rust with Homebrew's rustup 1.29.1 (keg-only); added export PATH="/opt/homebrew/opt/rustup/bin:$PATH" to ~/.zshrc. Pinned rustc 1.99.1 with clippy, rustfmt, and wasm32-unknown-unknown; uninstalled the duplicate stable toolchain.
- Created the repository, the Cargo workspace (dominofish-core, dominofish), the state files, scripts/check.sh, scripts/regress.sh, and the M0 verify scripts.
- dominofish selfplay plays random canonical legal moves to a finished game in both modes. The rules in core/src/rules.rs are written from the digest's rule list and a reading of the pinned Capi code; no rules fidelity is claimed until M1.
- Next: commit, mark the M0 features, then M1-01 (docs/RULES.md).

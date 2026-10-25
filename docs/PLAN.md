# DominoFish plan

The brief is docs/prompts/BUILD.md; this file holds the plan refined from it, the decisions with reasons, and the owner's answers word for word. Feature entries and their verify commands are in docs/features.json; progress is in docs/progress.md.

## Milestones

Each milestone ends with an exit feature, id M<k>-99, whose verify command runs the exit check. Features are taken in milestone order, then by id number, so a milestone's exit comes after the features appended to it. The exit conditions are BUILD.md's text, copied into docs/features.json.

| Milestone | Goal | Status |
|---|---|---|
| M0 | Bootstrap: toolchain, state files, check and regress scripts, random selfplay in both modes | passed 2026-10-25 |
| M1 | Rules parity with Capi: bitboard core, RULES.md, golden traces, edge cases, perft, differential fuzzer, property tests, 0 allocations, playout bench | in progress |
| M2 | Information sets, baseline bots, match harness with duplicate pairs and SPRT, REPORT.md | not started |
| M3 | Exact perfect-information solver, omniscient bot, dominofish bench | not started |
| M4 | PIMC with an exact consistent-world sampler: the strength-first engine | not started |
| M5 | Variance reduction (control variates) | not started |
| M6 | Game equity table W and calibrated win probability | not started |
| M7 | Exact endgames by enumeration | not started |
| M8 | Inference and partner modeling | not started |
| M9 | Strategy-fusion fixes, each by SPRT at equal compute | not started |
| M10 | Learned evaluation | not started |
| M11 | Analysis CLI (dominofish review) and release-ready docs | not started |

M2 to M4 are the strength-first path of owner decision 4: nothing outside it starts before M4 exits. No strength number is reported before the M1 exit (hard constraint 4).

## Architecture and API

Cargo workspace (decision D1):
- crates/dominofish-core, library: rules, information sets, sampling, search. No I/O, global state, threads, or clocks; no dependencies; builds for wasm32-unknown-unknown at every commit (check.sh).
- crates/dominofish, binary dominofish: the command line (selfplay now; match, bench, analyze, review, report later) and all process and file I/O.
- Planned: a cdylib crate for the wasm checksum (M1-10, one extern "C" export, no wasm-bindgen); a native crate that splits one call's worlds across threads and merges them to the one-thread result (hard constraint 1), when search exists.

Core state (M0, rules.rs): per-seat hand masks (u32, 28 bits in Capi's tile order), a board mask with left and right ends, the 1v1 boneyard as an ordered array popped from its end, turn, opener (starterThisRound), last player, one pass counter, scores, target, phase, and the next round's leader. The automatic round-1 tile and the opener tie-breaks follow reducer.ts:90-185 and need the deal's hand order, which the Deal type keeps.

Moves (provisional until docs/RULES.md at M1-01, decision D6): Play { tile, end }, Draw, Pass. On an empty board `end` selects the orientation (Left: as dealt, low pip left; Right: flipped). The canonical generator emits one move per tile on an empty board (as dealt), one per tile and fitting end otherwise, and only the left end when both ends show the same pip. Events are typed (Domino, Capicua, Tranque, PaseCorrido, PaseSalida) and map to Capi's callouts only in the parity harness.

Hot-path bounds (digest 1.8): at most 12 distinct plays (MoveList holds 16); a 1v1 hand holds at most 21 tiles; a round has at most 128 plies; the boneyard holds at most 14 tiles. Hot-path buffers are fixed-size arrays sized from these bounds.

Public API (planned, M2 on; hard constraint 1): InfoSet built from a game record and a seat (own hand, public history, hand sizes, boneyard count, scores, target, opener, derived voids, 1v1 opening deductions); Limits (worlds, nodes, memory cap); analyze(&InfoSet, &Limits, seed) -> Analysis (each legal move's value, the chosen move, information-set size, nodes). Full states stay in the referee, the simulator inside sampled worlds, and the omniscient bot.

## Method decisions

- D1, 2026-10-25. Two crates, core and command line. Reason: the no-I/O, no-clock, and wasm32 rules bind the core only; keeping every I/O dependency in the binary leaves the core with none. More crates join only when a milestone needs one.
- D2, 2026-10-25. Seeded randomness is SplitMix64 with Lemire's exact bounded integers, written in core/src/rng.rs. Reason: recorded seeds (fixtures, pre-registered runs) must keep their streams forever, and a crate upgrade could change a stream or its range reduction (rand changed its range sampling between 0.8 and 0.9). Checked: rand_core and rand_pcg would add a dependency and still need our own range reduction for stability.
- D3, 2026-10-25. The command line parses arguments with lexopt (see Dependencies). Checked: clap with derive pulls a proc-macro dependency tree and more build time for a few flags.
- D4, 2026-10-25. Property tests (M1-08) run over states reached by seeded random playouts, written as plain tests, not proptest. Reason: all randomness comes from explicit seeds (hard constraint 7) and a failing case is reproduced by its seed; shrinking a playout-generated state adds little; proptest would be a large dev-dependency tree.
- D5, 2026-10-25. Exit features use id M<k>-99 so that features appended when a milestone starts sort before its exit.
- D6, 2026-10-25. Canonical moves as stated under Architecture; final definition and the map from Capi intents in docs/RULES.md (M1-01).
- D7, 2026-10-25. Selfplay and matches deal with DominoFish's RNG; parity tests inject Capi's deals (hard constraint 2). A deal keeps hand order and boneyard order.
- D8, 2026-10-25. The tranque test is O(1): locked if and only if no tile outside the board shows either end (ALL & !board & (SUIT[L] | SUIT[R]) == 0). It equals Capi's isBoardLocked (validate.ts:45-51) because every tile is on the board, in a hand, or in the boneyard. The tranque geometry property (digest 1.4) is tested at M1-08.

- D9, 2026-10-25. The differential fuzzer (M1-07) runs Capi in Node as the driver and Rust as the verifier: fuzz.mjs streams Capi's canonical legal set before each move and a 32-bit digest of the canonical state and event after it, and `dominofish capi-fuzz` replays and compares. Reason: one stream per run instead of a round trip per move; Node is the bottleneck (about 3 minutes per 1,000,000 rounds). A digest collision could hide one mismatch with probability 2^-32 per differing move; a mismatch that diverges the state also changes the following moves' sets and digests. Fuzz seed 1 for the full and the fast run (the fast run is the first 10,000 rounds of the same stream).

## Rejected techniques

None measured yet. Each rejection will record its numbers here.

## Pre-registered runs

None yet. Every strength run records here, committed before it starts: engines and settings, mode and target, deal seed base and range, SPRT bounds, maximum pair count, and the expected wall time.

## Seed ranges

Seeds are u64. A run uses deal seeds base + i for pair i and records its base and maximum pair count here before it starts.

| Range | Use |
|---|---|
| 0 to 999,999,999 | tests, fixtures, parity, perft, benches, selfplay examples; never a strength result |
| 1,000,000,000 to 1,999,999,999 | dev deals: tuning and exploratory matches |
| 2,000,000,000 to 2,999,999,999 | held-out deals for strength claims (SPRT runs, README numbers) |
| 3,000,000,000 to 3,999,999,999 | held-out games for calibration and inference quality (M6 Brier, M8 sampling ratio); never used to build tables or train |
| 4,000,000,000 to 4,999,999,999 | training data (M6 W table, M10 nets) |

## Resource caps

- Threads: at most 4 for any run unless the owner says the machine is free; cargo uses [build] jobs = 4.
- Memory: peak RSS under 2 GB per engine process and 3 GB for all DominoFish processes; table and world-pool flags default to at most 256 MB per engine and refuse values over the cap; record peak RSS with /usr/bin/time -l.
- Disk: df -g /Users/Adelson before builds and long runs; below 8 GB free, cargo clean and delete raw logs already summarized in committed files; below 5 GB, stop and ask. One target tree (old-commit builds in target/old); no git worktrees.
- Runs: long runs start from the main session's background shell as nohup nice -n 10 <command> > runs/<name>.log 2>&1 &, with the PID, log path, and ETA in the progress top block; a run expected over 6 hours waits for the owner.

## Dependencies

| Crate | Used by | Version | License | Reason |
|---|---|---|---|---|
| lexopt | dominofish (binary) | 0.3.2 | MIT | Argument parsing with no dependencies of its own (D3) |

The core has no dependencies.

## Toolchain

Installed 2026-10-25 with Homebrew's rustup 1.29.1 (keg-only; PATH line added to ~/.zshrc), pinned in rust-toolchain.toml:
- rustc 1.99.1 (58cf8f9c3 2026-10-13)
- cargo 1.99.1 (5f94df478 2026-08-27)
- targets: aarch64-apple-darwin, wasm32-unknown-unknown; components: clippy, rustfmt

## Estimated sessions

| Milestone | Sessions |
|---|---|
| M0 | 1 |
| M1 | 3 to 5 |
| M2 | 3 to 4 |
| M3 | 2 |
| M4 | 3 |
| M5 | 2 |
| M6 | 2 |
| M7 | 2 |
| M8 | 3 |
| M9 | 3 to 4 |
| M10 | 4 to 5 |
| M11 | 2 |

## Out of scope

Each item waits on an owner decision (BUILD.md, Out of scope):
- Capi integration (bots and analysis in Capi): the server endpoint and Vercel plan (Hobby is non-commercial; Capi has ads and in-app purchases), deal-log storage while Capi's repo and game reads are public, per-seat masking of game_state, the PLAYBOOK.md:104 rule against claiming AI opponents, the store line "there are no computer opponents", wasm packaging and npm names, any on-device engine.
- Human benchmark: players, consent, a platform, and logging.
- Research tier (sequence-form LP or CFR endgames for 1v1, ReBeL or Student of Games search, team CFR for 2v2 endgames): a go-ahead and a compute budget.
- Publishing: GitHub repo and visibility, license, crates.io or npm. Until then: no LICENSE file, no license, license-file, repository, or homepage field, and publish = false in every crate.

## Decision log

Owner decisions, copied word for word from docs/prompts/BUILD.md (written and approved by the owner on 2026-10-25):

1. 2026-10-25. Name DominoFish, repository dominofish. Tagline "The Stockfish of Dominican Dominoes." Subtitle "An open-source Dominican dominoes AI engine." Credit line "Built by the creator of Capi." Inside Capi the feature reads "Powered by DominoFish." Resume line "DominoFish: Competitive Imperfect-Information Game AI Engine".
2. 2026-10-25. Standalone open-source repository, separate from Capi. Capi consumes it later as a dependency or a service.
3. 2026-10-25. Engine core in Rust.
4. 2026-10-25. Strength first. After the rules core, the first milestone is a measured engine that beats baseline bots. Capi bots and the analysis tool come later, on top of the engine.
5. 2026-09-26 to 2026-09-29. The rules are Capi's rules ("regla de patio"), decided after a research pass (/Users/Adelson/Desktop/personal/capi/docs/research/rules-2026-09/README.md). The engine reproduces them exactly as implemented in /Users/Adelson/Desktop/personal/capi/packages/engine and never "corrects" them. Making a rule configurable is fine only where cheap, with Capi's rule as the default.
6. Durable identifiers beyond item 1 are mine: crate names other than dominofish*, npm package names, domains, crates.io or npm publishing, GitHub repo creation and visibility. Ask first.

Questions sent to the owner on 2026-10-25 (answers go here word for word, dated):
- Q1. Create github.com/aaguasvivas/dominofish now, public or private until results exist?
- Q2. Which license?
- Q3. May long runs use more than 4 threads while I am away, and in which hours?
- Q4. Should a Stop hook or /goal condition enforce scripts/check.sh?

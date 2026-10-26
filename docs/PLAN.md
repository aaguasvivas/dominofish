# DominoFish plan

The brief is docs/prompts/BUILD.md; this file holds the plan refined from it, the decisions with reasons, and the owner's answers word for word. Feature entries and their verify commands are in docs/features.json; progress is in docs/progress.md.

## Milestones

Each milestone ends with an exit feature, id M<k>-99, whose verify command runs the exit check. Features are taken in milestone order, then by id number, so a milestone's exit comes after the features appended to it. The exit conditions are BUILD.md's text, copied into docs/features.json.

| Milestone | Goal | Status |
|---|---|---|
| M0 | Bootstrap: toolchain, state files, check and regress scripts, random selfplay in both modes | passed 2026-10-25 |
| M1 | Rules parity with Capi: bitboard core, RULES.md, golden traces, edge cases, perft, differential fuzzer, property tests, 0 allocations, playout bench | passed 2026-10-25 |
| M2 | Information sets, baseline bots, match harness with duplicate pairs and SPRT, REPORT.md | in progress |
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
- crates/dominofish, binary dominofish: the command line (selfplay, capi-fuzz, playout-bench now; match, bench, analyze, review, report later) and all process and file I/O.
- crates/dominofish-wasm, cdylib (since M1-10): one extern "C" export, playouts_checksum, for the native and wasm checksum; no wasm-bindgen.
- Planned: a native crate that splits one call's worlds across threads and merges them to the one-thread result (hard constraint 1), when search exists.

Core state (M0, rules.rs): per-seat hand masks (u32, 28 bits in Capi's tile order), a board mask with left and right ends, the 1v1 boneyard as an ordered array popped from its end, turn, opener (starterThisRound), last player, one pass counter, scores, target, phase, and the next round's leader. The automatic round-1 tile and the opener tie-breaks follow reducer.ts:90-185 and need the deal's hand order, which the Deal type keeps.

Moves (provisional until docs/RULES.md at M1-01, decision D6): Play { tile, end }, Draw, Pass. On an empty board `end` selects the orientation (Left: as dealt, low pip left; Right: flipped). The canonical generator emits one move per tile on an empty board (as dealt), one per tile and fitting end otherwise, and only the left end when both ends show the same pip. Events are typed (Domino, Capicua, Tranque, PaseCorrido, PaseSalida) and map to Capi's callouts only in the parity harness.

Hot-path bounds (digest 1.8): at most 12 distinct plays (MoveList holds 16); a 1v1 hand holds at most 21 tiles; a round has at most 128 plies; the boneyard holds at most 14 tiles. Hot-path buffers are fixed-size arrays sized from these bounds.

Public API (M2 on; hard constraint 1; the InfoSet exists since M2-01, D10): InfoSet built from a game record and a seat (own hand, public history, hand sizes, boneyard count, scores, target, opener, derived voids, 1v1 opening deductions); Limits (worlds, nodes, memory cap); analyze(&InfoSet, &Limits, seed) -> Analysis (each legal move's value, the chosen move, information-set size, nodes). Full states stay in the referee, the simulator inside sampled worlds, and the omniscient bot.

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

- D10, 2026-10-25. The InfoSet (M2-01, core/src/infoset.rs) is built from a GameRecord (core/src/record.rs: each round's deal and moves) and a seat by replaying to the point, and keeps only public facts and the seat's own hand: board and ends, turn, opener, last player, passes, scores, hand sizes, boneyard count, round 1's automatic tile, the round's public history (each move with the ends before it; draws as counts), current-hand voids, and the 1v1 round-1 exclusions of each dealt hand (opener rule R4). 1v1 voids are time-stamped: a draw's void binds the tiles held at the draw and its non-fitting tiles, and applies to the whole hand only after the drawer's next move (the drawn tile that fits is played then); older draws bind only older tiles, which the history keeps for the world counter (M2-03). The record holds hidden tiles; only the referee and record tools read it. Reason: one replay path for every consumer, and an equality test (records that differ only in hidden tiles give equal InfoSets) checks the no-hidden-tile rule of hard constraint 1.

- D11, 2026-10-25. Consistent worlds (M2-03, core/src/worlds.rs). A world is an assignment of the tiles the seat cannot place to holder groups of fixed sizes, each tile only where it may be. 2v2: the three other hands; a tile may be in a hand unless it shows one of that seat's void pips. 1v1: the other seat's acquisition batches (its dealt hand, then the non-fitting tiles of each of its draws) and the boneyard; the tiles are the hidden ones plus the other seat's played tiles (their batch is hidden), with each draw's fitting tile pinned (it is the play right after the draw, R15); a tile may be in a batch when it avoids the ends of every draw and pass of that seat while the batch held it, and the dealt batch also avoids the round-1 exclusions. Every assignment stands for the same number of full deal orders, so the count is proportional to the number of consistent deals. The count is a dynamic program over tiles keyed by the remaining group sizes. Evidence: the counts of the Leads, the 4-6 example, the true world allowed at 9,873 decisions, and equality with a brute-force oracle that builds every candidate deal and replays the record (150 positions per mode, 133 of the 1v1 ones after a draw of the other seat). Known limit for sampling (M4): when the other 1v1 seat opened round 1 with no double, its hand order chose the automatic tile among equal pip sums, so a world where it also holds another tile of that sum stands for fewer deal orders (1/m of them for m such tiles); the count does not weight this. The counter returns None while the other 1v1 seat's draw is unresolved; a seat's own decisions never fall there.

- D12, 2026-10-25. Baseline bots (M2-04, core/src/bots.rs), each deciding from its seat's InfoSet with ties broken by its RNG: random (uniform over canonical legal moves); heaviest, bota gorda (most pips); accompanied, la mas acompanada (the exposed pip is one it keeps the most tiles of, then pips; digest 3.5 and Moreno-Montiel 2020 name it the best simple heuristic); rule (scores ends the next seat is void in, avoids ends the partner is void in, counts its own and the unseen tiles that can follow, then pips). The in-process referee (referee::play_bots) builds each seat's InfoSet from the record so far and keys each seat's RNG by the round's deal seed and the seat, so a duplicate pair of identical bots replays the same choices (M2-08). These are fixed baselines; their strength is measured only by the harness (M2-06 to M2-09).

- D13, 2026-10-25. The match harness (M2-06, crates/dominofish/src/harness.rs and engine.rs). Every seat is a separate engine process over the line protocol, so 2v2 partners share nothing; the referee sends each engine the InfoSet it builds for that seat before each go. Binaries come from scripts/build-bin.sh (git archive of the commit, built in target/old, copied to runs/bin/<commit>/); the harness refuses target/. A duplicate pair plays one deal seed twice with the sides swapping seats, deals derived per round from the seed so full games share them, and seat seeds keyed by pair and seat, which makes identical deterministic bots score exactly 0 per pair. Amended 2026-10-25 after the M2 review: seat seeds come from a separate engine seed base (`match --engine-seed`, required), derive(derive(base, pair), 100 + seat); the header sent to engines holds no seed and carries the budgets, threads, and tables each side's engines report in their ready line; the deal seeds and the engine seed base go only to the results file. Before this, the header held the deal seeds and the seat seed was derive(deal seed, 100 + seat), which an engine could invert to rebuild every hidden tile. Each reply has one deadline, and a line other than info that waits before a go is a protocol fault. A per-move timeout (default 10 s) is a safety limit; budgets are nodes or worlds (hard constraint 7). Binary identity uses FNV-1a 64 of the file, written here instead of adding a hash crate because it is an identity check, not a security one.

- D14, 2026-10-25. The SPRT (M2-07, crates/dominofish/src/stats.rs) is a normal-approximation GSPRT on per-pair results with the sample variance (after the Fishtest normalized GSPRT idea, digest 2.3), alpha = beta = 0.05, Wald bounds, and no decision before 16 pairs; `dominofish match --sprt mu0,mu1` runs it on the pair margin in points and stops at a decision. Confidence intervals are normal 95% intervals over pairs: points per round (pair margin over its rounds) and game-win rate (pair wins over 2). T1 checks the error rates on synthetic data. Game-win SPRTs (M6) and Elo are not used in M2.

## Rejected techniques

None measured yet. Each rejection will record its numbers here.

## Pre-registered runs

Every strength run records here, committed before it starts: engines and settings, mode and target, deal seed base and range, SPRT bounds, maximum pair count, and the expected wall time. Statistical tests of the harness itself are stated here the same way.

- T1, M2-07 synthetic SPRT test, stated 2026-10-25 and committed before its first run. The SPRT of crates/dominofish/src/stats.rs is a normal-approximation GSPRT on per-pair results: LLR = (mu1 - mu0) / s^2 * (sum - n (mu0 + mu1) / 2) with s^2 the sample variance, decisions from the 16th pair on, alpha = beta = 0.05, accept H0 at LLR <= ln(beta / (1 - alpha)) and H1 at LLR >= ln((1 - beta) / alpha). Configurations: (a) per-pair results Normal with SD 40, H0 mean 0, H1 mean 4; (b) a fixed empirical distribution of 2,000 per-pair margins of duplicate 2v2 first rounds, rule bot against accompanied bot (in process, deal seeds 7,100,000 + i), centered to mean 0 and shifted to the true mean, with H0 mean 0 and H1 mean 0.1 times its SD. Each configuration: 4,000 runs with the true mean at H0 and 4,000 at H1, run seeds 7,000,000 + k, at most 20,000 pairs per run. Pass if in each configuration the share of H0 runs that accept H1 and the share of H1 runs that accept H0 each lie in 0.05 +- 3 * sqrt(0.05 * 0.95 / 4000) = [0.0397, 0.0603], and under 1% of all runs reach the maximum. Amended 2026-10-25 after the M2 milestone review, before any rerun: (1) the code has used run seeds 7,000,000 + 1,000,000 c + 100,000 h + k (c = 0 normal, 1 empirical; h = 0 true mean at H0, 1 at H1; run k) since its first commit (ea2ac29, after this entry in 979c3ee), not 7,000,000 + k as stated; the first and only run used them, so no seed changed after a result (Q6). (2) Configuration (b) draws its 2,000 margins from the accompanied bot as fixed after the review (D12's pip for a double on the left end), so reruns of M2-07 use that bot. The first result stays as recorded in docs/progress.md.
- T2, M2-08 null SPRT test, stated 2026-10-25 and committed before its first run. 200 runs of `dominofish match --unit round --target 100 --sprt 0,10 --pairs 2000`, 100 in 1v1 and 100 in 2v2, between two copies of the random bot seeded independently (run k: side A `engine --bot random --salt <2k+1>`, side B `--salt <2k+2>`; each engine mixes its salt into the seat seed), deal seeds 9,000,000 + 10,000 k + i for pair i, with HEAD's runs/bin binary. Under the null both sides are the same bot, so the true mean pair margin is 0. Pass if the number of runs that accept H1 lies in [3, 19], the central 99% interval of Binomial(200, 0.05) (P(X < 3) = 0.0023, P(X > 19) = 0.0027), fewer than 5% of runs end inconclusive at 2,000 pairs, and no run has an illegal move, crash, or timeout. Expected wall time: about 4 minutes (about 130 pairs per run). Amended 2026-10-25 after the M2 milestone review, before any rerun: the harness now requires an engine seed base (D13); every T2 run uses --engine-seed 5,000,000,081, and each engine still mixes its salt into its seat seed, so the two sides stay independently seeded. The first result (H1 in 12 of 200 runs) stays as recorded (Q6).
- R1, M2-09 baseline table, stated 2026-10-25 and committed before its first run (scripts/run-baselines.sh). Every pairing of the four baseline bots, side A first: heaviest against random, accompanied against random, rule against random, accompanied against heaviest, rule against heaviest, rule against accompanied; each in 1v1 and 2v2; as rounds (1,000 pairs) and as full games at target 100 (300 pairs) and at target 200 (300 pairs): 36 rows, row k in that order (pairing, then mode, then unit). Held-out deal seeds 2,000,000,000 + 100,000 k + i for pair i. Both sides: the runs/bin binary of the commit that holds this entry (HEAD at the run), Bench none, no budgets (bots), 1 thread, no tables, a 10 s per-move safety timeout. SPRT on the pair margin, H0 mean 0, H1 mean 4 points per pair, alpha = beta = 0.05, in tracking mode: every row plays all its pairs and reports the first boundary crossing, or inconclusive. Results: docs/eval/results/2026-10-25-baselines/, then dominofish report. Self-play results are relative only. Expected wall time: about 4 minutes.

## Seed ranges

Seeds are u64. A run uses deal seeds base + i for pair i and records its base and maximum pair count here before it starts.

| Range | Use |
|---|---|
| 0 to 999,999,999 | tests, fixtures, parity, perft, benches, selfplay examples; never a strength result |
| 1,000,000,000 to 1,999,999,999 | dev deals: tuning and exploratory matches |
| 2,000,000,000 to 2,999,999,999 | held-out deals for strength claims (SPRT runs, README numbers) |
| 3,000,000,000 to 3,999,999,999 | held-out games for calibration and inference quality (M6 Brier, M8 sampling ratio); never used to build tables or train |
| 4,000,000,000 to 4,999,999,999 | training data (M6 W table, M10 nets) |
| 5,000,000,000 to 5,999,999,999 | engine seed bases (`match --engine-seed`), chosen apart from any deal seed and never computed from one |

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
- Q5 (added 2026-10-25 during M2). May a local git pre-commit hook (.git/hooks/pre-commit, not versioned) run scripts/check.sh before each commit? Two commits this session failed check.sh (31443cd, 2c11b62) because the commit was not gated on it.
- Q6 (added 2026-10-25 at the M2 exit). The M2 review found that T1's code seeds differ from T1's text (7,000,000 + 1,000,000 c + 100,000 h + k, against 7,000,000 + k), and its fixes changed one input of each pre-registered test: T1's configuration (b) now uses the fixed accompanied bot, and T2 now needs an engine seed base (5,000,000,081). The amendments are in the T1 and T2 entries, committed before the reruns. Do you accept the amended entries, or do you want new tests stated instead?

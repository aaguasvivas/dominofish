<p align="center">
  <img src="docs/assets/dominofish.svg" alt="DominoFish logo: a 1-6 domino tile drawn as a fish, with the 1 as its eye" width="240">
</p>

<h1 align="center">DominoFish</h1>

<p align="center">
<b>
The Stockfish of Dominican Dominoes.
</b>
<br>
An open-source Dominican dominoes AI engine.
<br>
<sub>
Built by the creator of Capi.
</sub>
</p>

<p align="center">
<a href="https://dominofish.adelsonaguasvivas.workers.dev"><b>Play now</b></a> ·
<a href="#build-and-run">Build and run</a> ·
<a href="#web-board">Web board</a> ·
<a href="#what-it-does-now">What it does now</a> ·
<a href="#results">Results</a> ·
<a href="#testing">Testing</a> ·
<a href="#architecture">Architecture</a> ·
<a href="#roadmap">Roadmap</a>
</p>

DominoFish is a Rust engine for Dominican dominoes, 1v1 and 2v2 (parejas), under the rules of the game Capi. It aims to find the best move and a calibrated win probability from one player's information alone. This version plays from one seat's information with PIMC: it samples the hands that seat cannot see, solves each sampled deal exactly, and plays the move with the best average. Under it sit the rules core, the information sets, an exact sampler, an exact solver, four baseline bots, the match harness that measures them, and a web board where you play the bots in your browser. The harness also corrects each match result for part of its luck, with AIVAT control variates (M5). A calibrated win probability (M6) comes next.

| Area | Where it stands |
|---|---|
| **Rules** | 0 mismatches with Capi's engine over 117,456 golden-trace moves, 87 edge scenarios, 200 perft queries, and 1,000,000 fuzzed rounds per mode |
| **Speed** | About 1.4 million random 1v1 rounds and 1.3 million 2v2 rounds per second on one core (commit de3109a, load average 13.9), with 0 heap allocations in move generation, play, and undo |
| **Solver** | Exact for a known deal: 0 mismatches against naive minimax on every position checked; `dominofish bench` solves 32 full rounds in under 1.1 s on one core (Bench 6,121,081 nodes) |
| **Engine** | PIMC at 64 worlds per move beats every baseline by SPRT in all 16 rows of R4; against the rule bot it gains 8.98 [4.43, 13.54] points per round in 2v2 rounds and wins 0.771 [0.695, 0.833] of its 2v2 games to 100 |
| **Sampling** | Exact: on 360 positions small enough to enumerate, the sampled deals match the exact distribution (chi-square), 1v1 boneyard orders and openings without a double included, and the true placement of the unseen tiles comes up at the uniform rate |
| **Information** | Every engine runs as its own process and sees only its own seat's information |
| **Variance reduction** | AIVAT control variates: no bias detected in the 14 rows of test T7 (corrected minus raw within 1.732 standard errors of 0), and the SD of a result falls by a factor of up to 1.19 |
| **Best baseline** | The rule bot beats the other baselines by SPRT in 16 of 18 R2 rows and gains 11.99 [10.69, 13.29] points per round on the random bot in 1v1 rounds |

## Build and run

You need Rust 1.99.1, pinned in `rust-toolchain.toml`.

```bash
cargo build --release
```

Play one game of random legal moves and print every hand, move, and score:

```bash
target/release/dominofish selfplay --mode 2v2 --seed 1
```

```text
DominoFish selfplay   mode 2v2   seed 1   target 100

Round 1
  Hands     n  [0|6] [0|4] [0|0] [5|6] [1|5] [2|6] [0|1]
            e  [5|5] [6|6] [3|6] [4|6] [2|5] [2|4] [3|3]
            s  [1|4] [1|1] [4|5] [0|3] [0|5] [2|3] [4|4]
            w  [1|3] [1|2] [3|4] [0|2] [2|2] [1|6] [3|5]
  e   opens   [6|6]
  s   passes
  w   plays   [1|6] left   pase de salida, EW +25             NS   0   EW  25
  n   plays   [1|5] left
  ...
  n   plays   [0|0] left   tranque, n 19 pips, e 7 pips, EW +44   NS   0   EW  69
  Board     [0|0][0|6][6|3][3|4][4|2][2|0][0|3][3|3][3|5][5|1]
            [1|6][6|6][6|4][4|4][4|0][0|1][1|4][4|5][5|5][5|0]
  ...

Final   NS 109   EW  69   NS wins in 3 rounds, 68 moves
```

Save a game as a record, then ask PIMC to rank the moves of the seat on turn after the first three moves of round 1. It reads only what that seat can see:

```bash
target/release/dominofish selfplay --mode 2v2 --seed 1 --record game.txt
```

```bash
target/release/dominofish analyze --record game.txt --round 1 --ply 3
```

```text
DominoFish analyze   mode 2v2   target 100   round 1, ply 3   seat e (team EW)
PIMC, sampled: 64 worlds of 10,570,560 consistent   seed 1   nodes 7,700,723   capped solves 0

Move                   Points      SE
play 5-5 left          -13.88    4.09
play 2-5 left          -36.28    4.51
play 3-6 right         -40.14    4.38
play 4-6 right         -43.08    4.38

Points: team EW's round margin from this move on, its own points included, averaged over the sampled worlds.
Best: play 5-5 left
```

Run every check: formatting, lint, all tests, the wasm32 build, the report check, Bench determinism (`dominofish bench` twice), and an em dash scan.

```bash
scripts/check.sh
```

`target/release/dominofish --help` lists every command, and `dominofish <command> --help` gives its options. `dominofish bench` solves the 32 bench positions and prints `Bench: <nodes>`, as Stockfish does.

## Web board

Play Dominican dominoes at [dominofish.adelsonaguasvivas.workers.dev](https://dominofish.adelsonaguasvivas.workers.dev), in English or Spanish, 1v1 or 2v2, against DominoFish or the four baseline bots. Ask DominoFish for a hint: it sees only your hand and the public moves, samples 32 deals of the hands you cannot see, and ranks your moves by their mean points with a sampling error. Or turn on the exact analysis: the solver scores every legal move for the rest of the round. The solver sees every hand, so the board labels its values as perfect information. The engine runs in your browser as WebAssembly; your games never leave the page.

<p align="center">
  <img src="docs/assets/board.png" alt="The DominoFish web board in dark mode: a 2v2 round on green felt, your hand below, DominoFish's hint ranking your moves, and the exact analysis scoring each legal move" width="800">
</p>

To run it locally, build it into one self-contained page, then open the page in a browser:

```bash
scripts/build-web.sh
```

```bash
open web/dist/index.html
```

## What it does now

- **Rules.** It plays Capi's rules as Capi's engine implements them: the automatic first tile (the 6-6 when someone holds it), draws in 1v1, pase corrido and pase de salida with their cap, capicúa, and tranque with ties to the opener. [docs/RULES.md](docs/RULES.md) cites every rule to Capi's source.
- **Information sets.** It builds what one seat knows: its hand, the public moves, hand sizes, voids from passes, the 1v1 draw history, and round-1 deductions. It counts exactly the ways to place the unseen tiles that agree with that knowledge (one known limit, for the 1v1 opening without a double, is in docs/PLAN.md D11).
- **Baseline bots.** Random, heaviest tile (bota gorda), most-accompanied tile (la más acompañada), and a rule bot that reads pass voids.
- **Exact solver.** For a known deal it finds the rest of the round's exact margin with alpha-beta search and a transposition table whose key covers the bonus cap. It drives the omniscient bot, a cheating upper bound, and solves every world that PIMC samples.
- **Sampling.** It draws placements of the unseen tiles in exact proportion to the deals each one stands for, from a count over the tiles. It never falls back to the true hands, which it cannot see. It draws again under one rule only: after a 1v1 opening without a double, a placement that gives the opener several tiles of the opening tile's pip sum stands for fewer deals ([docs/PLAN.md](docs/PLAN.md) D17).
- **PIMC.** For each legal move it averages the exact result over the same sampled worlds, 64 by default, and plays the best mean. `dominofish analyze` prints that ranking, with its sampling error, for any point of a game record.
- **Match harness.** It plays duplicate pairs: one deal played twice, with the sides swapping seats. Every seat runs as its own process and gets only its own information; engine seeds come from a base apart from the deal seeds, and no engine sees a deal seed. The harness reports 95% confidence intervals and runs a sequential probability ratio test (SPRT).
- **Variance reduction.** The referee sees every hand, so it can value part of each match's luck with the exact solver: each new deal after round 1, each 1v1 draw when no engine sees the hidden tiles, and each random choice a baseline bot makes among its tied best moves. It subtracts these terms from the result with coefficients fitted on the other half of the pairs, which keeps the result's mean (AIVAT control variates, [docs/PLAN.md](docs/PLAN.md) D19). `dominofish match --aivat 8` reports the corrected result beside the raw one.

## Results

### PIMC, the strength-first engine

PIMC plays from one seat's information. For each move it samples 64 deals that agree with everything its seat has seen, solves every legal move in each deal with the exact solver, and plays the best average. In run R4 (commit ebaa5fd, pre-registered in [docs/PLAN.md](docs/PLAN.md), held-out deals) it beats every baseline by SPRT in all 16 rows. Against the rule bot, the strongest baseline:

| Mode | Unit | Pairs | Points per round [95% CI] | Game-win rate [95% CI] | SPRT (H0 0, H1 4) |
|---|---|---|---|---|---|
| 1v1 | round | 181 | 4.94 [1.77, 8.12] | - | H1 at pair 181 |
| 1v1 | game to 100 | 74 | 8.76 [6.36, 11.16] | 0.743 [0.667, 0.807] | H1 at pair 74 |
| 2v2 | round | 129 | 8.98 [4.43, 13.54] | - | H1 at pair 129 |
| 2v2 | game to 100 | 70 | 18.47 [13.80, 23.14] | 0.771 [0.695, 0.833] | H1 at pair 70 |

Settings: both sides from commit ebaa5fd (Bench 6,121,081), one thread each; PIMC at 64 worlds per move, a node cap of 10,000,000 per solve, and a 1 MB table; target 100; deal seeds 2,020,000,000 + 100,000 k + i for pair i of row k, engine seed base 5,000,400,000 + k; a 120 s safety limit per move. Command: `scripts/run-pimc.sh R4`. Results: docs/eval/results/2026-10-26-pimc-r4/; every row, with its wall time and peak RSS (18 MB per match), is in [docs/eval/REPORT.md](docs/eval/REPORT.md). PIMC's time per move at 64 worlds, from `dominofish pimc-stats` over 20 random games per mode, two runs each (commit ec9d372, one thread, load average 7.2 at the start): a 90th percentile of 0.73 to 0.75 s in 2v2 and 0.24 to 0.25 s in 1v1.

**Strength against worlds per move (R5).** PIMC at 1 to 64 worlds per move against the rule bot, rounds at target 100, 500 pairs per point, no SPRT, every point on the same held-out deals (seeds 2,030,000,000 + i in 1v1, 2,031,000,000 + i in 2v2), commit e3f9a4f (Bench 6,121,081), command `scripts/run-pimc.sh R5`:

| Worlds per move | 1v1 points per round [95% CI] | 2v2 points per round [95% CI] |
|---|---|---|
| 1 | -1.08 [-2.96, 0.81] | 0.99 [-1.76, 3.75] |
| 2 | 2.31 [0.50, 4.12] | 3.97 [1.27, 6.67] |
| 4 | 1.85 [0.10, 3.59] | 5.13 [2.57, 7.70] |
| 8 | 3.80 [2.11, 5.48] | 8.40 [5.87, 10.94] |
| 16 | 4.88 [3.21, 6.55] | 8.92 [6.41, 11.42] |
| 32 | 6.01 [4.28, 7.74] | 12.71 [10.25, 15.17] |
| 64 | 6.75 [5.00, 8.49] | 10.41 [7.87, 12.94] |

With one world, PIMC plays as if one guess at the hidden hands were true, and it does no better than the rule bot. More worlds bring more points. In 2v2 the intervals at 32 and 64 worlds overlap, so these runs show no gain from 32 to 64.

**The gap to the omniscient bot (R6).** The omniscient bot sees every hand and beats PIMC at 64 worlds by 24.22 [22.92, 25.52] points per round in 1v1 and 48.04 [46.34, 49.74] in 2v2 (rounds at target 100, 500 pairs per mode, no SPRT, deal seeds 2,040,000,000 + 100,000 k + i, commit e3f9a4f, command `scripts/run-pimc.sh R6`). It cheats, so the gap is a reference, never a target.

### The baselines

The rule bot against the random bot, from run R2 on held-out deals (commit d276826, pre-registered in docs/PLAN.md). Points per round are the rule bot's points minus the random bot's; the game-win rate is the rule bot's; the SPRT tests a mean pair margin of 0 against 4 points. [docs/eval/REPORT.md](docs/eval/REPORT.md) holds all 36 rows of R2, with every pairing of the four bots, and the 36 rows of the first run, R1. R1's rows with the accompanied bot measured a version with a scoring error, fixed before R2.

| Mode | Unit | Pairs | Points per round [95% CI] | Game-win rate [95% CI] | SPRT (H0 0, H1 4) |
|---|---|---|---|---|---|
| 1v1 | round | 1,000 | 11.99 [10.69, 13.29] | - | H1 at pair 56 |
| 1v1 | game to 100 | 300 | 14.84 [13.60, 16.08] | 0.870 [0.841, 0.895] | H1 at pair 38 |
| 2v2 | round | 1,000 | 12.22 [10.45, 13.99] | - | H1 at pair 100 |
| 2v2 | game to 100 | 300 | 16.92 [14.84, 19.00] | 0.758 [0.723, 0.791] | H1 at pair 84 |

These rows compare bots with bots. They measure nothing against human players.

### The omniscient upper bound

The omniscient bot sees every hand and plays the exact solver's move. It cheats on purpose: it marks the ceiling that a fair engine can approach, and it is never a goal. In run R3 (commit f77e9c2, pre-registered) it beats every baseline by SPRT in all 16 rows and wins every full game. Against the rule bot:

| Mode | Unit | Pairs | Points per round [95% CI] | Game-win rate [95% CI] | SPRT (H0 0, H1 4) |
|---|---|---|---|---|---|
| 1v1 | round | 16 | 26.25 [20.84, 31.66] | - | H1 at pair 16 |
| 1v1 | game to 100 | 16 | 31.39 [27.47, 35.31] | 1.000 [0.893, 1.000] | H1 at pair 16 |
| 2v2 | round | 16 | 54.47 [46.62, 62.32] | - | H1 at pair 16 |
| 2v2 | game to 100 | 16 | 62.52 [55.57, 69.48] | 1.000 [0.893, 1.000] | H1 at pair 16 |

R3 stops each row at its SPRT decision, so these intervals rest on 16 pairs each.

### Variance reduction (AIVAT)

`dominofish match --aivat 8` values the luck of a match with the exact solver (the terms under [What it does now](#what-it-does-now)). A deal or draw term is the value after the event minus its mean over 8 sampled alternatives (other deals, other boneyard orders), and an action term is the value after a bot's move minus its mean over the bot's tied moves, so each term has mean 0. The harness subtracts the terms from the result with coefficients fitted on the other half of the pairs. Test T7, pre-registered in [docs/PLAN.md](docs/PLAN.md), checks on 14 matchups that the correction keeps each result's mean, and it measures how much the correction narrows the result:

| Row | Mode | Unit | A | B | Pairs | Raw points per round [95% CI] | Corrected [95% CI] | Corrected minus raw, t | SD raw / corrected |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1v1 | round | rule | rule | 1,000 | 0.02 [-0.31, 0.34] | -0.02 [-0.35, 0.30] | -0.335 | 1.01 |
| 1 | 1v1 | round | random | random | 1,000 | 0.46 [-0.71, 1.62] | 0.65 [-0.33, 1.63] | 0.600 | 1.18 |
| 2 | 1v1 | round | rule | random | 16,000 | 12.44 [12.13, 12.76] | 12.38 [12.10, 12.65] | -0.794 | 1.16 |
| 3 | 1v1 | round | accompanied | heaviest | 16,000 | 2.71 [2.47, 2.96] | 2.73 [2.51, 2.95] | 0.242 | 1.11 |
| 4 | 1v1 | round | pimc (8 worlds) | rule | 300 | 4.50 [2.11, 6.88] | 4.39 [2.39, 6.40] | -0.146 | 1.19 |
| 5 | 1v1 | game to 100 | rule | random | 4,000 | 16.16 [15.82, 16.51] | 16.21 [15.91, 16.51] | 0.532 | 1.14 |
| 6 | 1v1 | game to 100 | rule | rule | 4,000 | 0.05 [-0.06, 0.16] | 0.06 [-0.05, 0.16] | 0.185 | 1.10 |
| 7 | 2v2 | round | rule | rule | 1,000 | 0.57 [0.13, 1.00] | 0.48 [0.06, 0.90] | -1.199 | 1.03 |
| 8 | 2v2 | round | random | random | 1,000 | -0.05 [-1.76, 1.65] | -0.87 [-2.30, 0.57] | -1.732 | 1.19 |
| 9 | 2v2 | round | rule | random | 16,000 | 12.08 [11.64, 12.52] | 12.17 [11.77, 12.57] | 0.887 | 1.11 |
| 10 | 2v2 | round | accompanied | heaviest | 16,000 | 3.90 [3.55, 4.24] | 4.00 [3.68, 4.31] | 1.548 | 1.08 |
| 11 | 2v2 | round | pimc (8 worlds) | rule | 300 | 6.71 [3.21, 10.21] | 6.61 [3.11, 10.11] | -0.682 | 1.00 |
| 12 | 2v2 | game to 100 | rule | random | 1,000 | 15.22 [14.04, 16.40] | 14.88 [13.80, 15.96] | -1.375 | 1.09 |
| 13 | 2v2 | game to 100 | rule | rule | 1,000 | 0.19 [-0.22, 0.60] | 0.24 [-0.17, 0.65] | 0.926 | 1.02 |

Corrected minus raw stays within 1.732 standard errors of 0 in every row, against a limit of 3.29 per row (two-sided, 0.001), and the sum of t² over the 14 rows is 12.33, against a limit of 36.12 (the 0.999 quantile of chi-square with 14 df). In rows 0, 1, 6, 7, 8, and 13 a bot plays itself, so the true mean is 0, and the corrected means lie within 2.24 standard errors of 0. Row 7's raw and corrected intervals both miss 0; with 6 such rows, at least one interval at 95% misses 0 with a probability of about 0.26. The test can also catch a bias. T5 and T6, the first two tests, used a version that divided each pair's terms by its number of rounds, which biases full games: a lucky side reaches 100 in fewer rounds. T6's row 5 exposed that bias, and on T7's games the same version gives t -5.085 in row 5.

The SD ratio gives the gain. At 1.14, as in row 5, a corrected result needs 1.31 times fewer pairs for the same interval width; at 1.19, as in rows 4 and 8, it needs 1.41 times fewer. AIVAT needed more than 10 times fewer hands in no-limit poker (Burch, Schmid, Moravcik, and Bowling, AAAI 2018). One reason for the smaller gain here, not verified: the solver values each position for perfect play afterwards, which the bots do not play ([docs/PLAN.md](docs/PLAN.md) D19). The SPRT stays on the raw pair margin. R1 to R6 came before M5 and report raw results only; from M5 on, every strength run reports both.

Settings: both sides from commit ff3cdea (Bench 6,121,081), one thread each; PIMC at 8 worlds per move in rows 4 and 11; `--aivat 8`; target 100; deal seeds 640,000,000 + 1,000,000 k + i for pair i of row k, from the test range, so no row here is a strength result; engine seed base 5,000,900,000 + k; no SPRT; a 120 s safety limit per move. Command: `scripts/run-t7.sh` (71 minutes at 4 rows at once). Results: docs/eval/results/2026-10-30-aivat-t7/, every pair with its terms; every row is also in [docs/eval/REPORT.md](docs/eval/REPORT.md).

### Reproduce

To reproduce the first pairs of every README row (2 to 30 pairs, the same seeds and budgets, about 2 minutes), run:

```bash
scripts/readme-fast.sh --check
```

It builds commits d276826 (R2), f77e9c2 (R3), ebaa5fd (R4), e3f9a4f (R5 and R6), and ff3cdea (T7) and compares 42 summary lines with the `-fast.txt` files in [docs/eval/results/](docs/eval/results/). To rerun a whole run, then its report:

```bash
scripts/run-pimc.sh R4
```

```bash
scripts/run-baselines.sh
```

```bash
target/release/dominofish report
```

To judge T7 again from its committed pairs (about 5 seconds):

```bash
scripts/verify/M5-03.sh
```

## Testing

- **Capi parity.** The Rust core replays 117,456 golden-trace moves from Capi's engine, 87 edge scenarios, and 200 perft queries, with 0 mismatches. A lockstep fuzzer compares the legal moves and a 32-bit digest of the full state with Capi's after every move: 0 mismatches over 1,000,000 rounds per mode.
- **Never-peeks.** Rearranging tiles a seat cannot see, behind the same public record, leaves its information set byte-identical, every bot's move unchanged, and PIMC's analysis unchanged down to its node counts.
- **Sampler.** On 230 positions small enough to enumerate (100 per mode and 30 openings without a double), a chi-square test at a fixed seed finds the sampled deals in the exact proportions. Over 53,259 decisions, 100 draws each, the true placement of the unseen tiles came up 167,890 times against 168,195.9 expected. On 100 more 1v1 positions the boneyard orders match their exact distribution too, and so do the placements after 30 openings without a double, where the weights differ (test T4). Two broken samplers, one without the boneyard shuffle and one without that weight, fail these tests. Every sampled deal replays the public record exactly.
- **SPRT.** On synthetic data its error rates match alpha and beta (0.05). Of 200 null runs between two copies of one bot, 12 accepted H1, where 10 is the expected count.
- **AIVAT.** On 14 matchups, test T7 detects no bias in the corrected results (the limit is 3.29 standard errors per row), and a version with a known bias fails the same test ([Variance reduction](#variance-reduction-aivat)).
- **Solver.** The solver equals naive minimax on every position of the first two rounds of 30 seeded games per mode wherever naive minimax finishes, including scores where a bonus pays or not, and its table on and off agree everywhere else.
- **Every commit.** `scripts/check.sh` runs formatting, lint, all tests, the wasm32 build, the report check, Bench determinism, and an em dash scan. `scripts/regress.sh` reruns the fast check of every passing feature.

## Architecture

```text
crates/dominofish-core   rules, information sets, world counts and sampling, bots,
                         exact solver, PIMC
                         (no I/O, threads, clocks, or dependencies; builds for wasm32)
crates/dominofish        the dominofish command and the match harness
crates/dominofish-wasm   the wasm module: the web board's engine, the playout checksum
web/                     the web board (HTML, CSS, JavaScript; no dependencies)
tools/capi-oracle        runs Capi's engine as the rules oracle (reads Capi, never writes)
docs/                    rules, formats, plan, progress, and the evaluation report
```

[docs/FORMATS.md](docs/FORMATS.md) defines the game records, the engine protocol, and the result files. [docs/PLAN.md](docs/PLAN.md) records every method decision with its reason.

## Roadmap

| Milestone | Goal | Status |
|---|---|---|
| M0 | Toolchain, checks, random selfplay | passed |
| M1 | Rules parity with Capi | passed |
| M2 | Information sets, baseline bots, match harness | passed |
| M3 | Exact perfect-information solver, omniscient bot | passed |
| M4 | Sampling over hidden hands, each world solved exactly | passed |
| M5 | Variance reduction for smaller measured gains | in progress |
| M6 | Game equity and a calibrated win probability | planned |
| M7 | Exact endgames | planned |
| M8 | Inference from plays and passes, partner modeling | planned |
| M9 | Fixes for strategy fusion | planned |
| M10 | Learned evaluation | planned |
| M11 | Analysis command and release docs | planned |

## Limitations

- No human benchmark yet: no result here involves a person.
- Self-play results are relative to these bots.
- 2v2 is a team game, and DominoFish makes no equilibrium claim for it.
- The baseline bots are fixed heuristics.
- PIMC solves each sampled world as if every seat could see every hand. So in 2v2 it treats its partner as a player who sees all hands, and in 1v1 it treats the boneyard order as known; M8 and M9 work on this.
- PIMC's time per move has a long tail in 1v1. At 64 worlds its 90th percentile is 0.25 s, but in R4 single moves took up to 119 s, in positions where many solves reach the node cap, while the machine's load average reached 96 (one measurement).

## License

DominoFish is licensed under either of the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.

# DominoFish

The Stockfish of Dominican Dominoes.

An open-source Dominican dominoes AI engine.

Built by the creator of Capi.

DominoFish is a Rust engine for Dominican dominoes, 1v1 and 2v2 (parejas), under the rules of the game Capi. Its goal is the best move and a calibrated win probability from one player's information. This version holds the rules core, the information sets, four baseline bots, and the match harness that measures them. The search engines come next: an exact solver (milestone M3), then sampling over hidden hands (M4). See [docs/PLAN.md](docs/PLAN.md).

## What it does now

- Plays Capi's rules as Capi's engine implements them: the automatic 6-6, draws in 1v1, pase corrido and pase de salida with their cap, capicua, and tranque with ties to the opener. [docs/RULES.md](docs/RULES.md) cites every rule to Capi's source.
- Builds what one seat knows: its hand, the public moves, hand sizes, voids from passes, the 1v1 draw history, and round-1 deductions. It gives the exact count of hidden deals that agree with that knowledge.
- Includes four baseline bots: random, heaviest tile (bota gorda), most-accompanied tile (la mas acompanada), and a rule bot that reads pass voids.
- Runs matches of duplicate pairs, one deal played twice with the sides swapping seats. Every seat runs as its own process and gets only its own information. The harness reports 95% confidence intervals and runs a sequential probability ratio test (SPRT).

## Results

The rule bot against the random bot, from run R1 on held-out deals (commit a3e108a, pre-registered in docs/PLAN.md). [docs/eval/REPORT.md](docs/eval/REPORT.md) holds all 36 rows, with every pairing of the four bots.

| Mode | Unit | Pairs | Rule bot points per round, mean [95% CI] | Rule bot game-win rate [95% CI] | SPRT, H0 0 against H1 4 points per pair |
|---|---|---|---|---|---|
| 1v1 | round | 1,000 | 10.77 [9.55, 11.98] | - | H1 at pair 54 |
| 1v1 | game to 100 | 300 | 14.63 [13.43, 15.83] | 0.873 [0.847, 0.900] | H1 at pair 26 |
| 2v2 | round | 1,000 | 9.78 [8.04, 11.52] | - | H1 at pair 143 |
| 2v2 | game to 100 | 300 | 16.47 [14.20, 18.75] | 0.763 [0.730, 0.796] | H1 at pair 57 |

These rows compare bots with bots. They measure nothing against human players.

To reproduce the first pairs of each row (10 to 30 pairs, the same seeds, under a minute), run:

```bash
scripts/readme-fast.sh --check
```

It builds commit a3e108a and compares four summary lines with [docs/eval/results/2026-10-25-baselines-fast.txt](docs/eval/results/2026-10-25-baselines-fast.txt). To rerun the whole table and its report:

```bash
scripts/run-baselines.sh
```

```bash
target/release/dominofish report
```

## Architecture

- `crates/dominofish-core`: rules, information sets, world counting, and bots. It has no I/O, threads, or clocks, takes no dependencies, and builds for wasm32.
- `crates/dominofish`: the `dominofish` command (selfplay, engine, match, report, capi-fuzz, playout-bench).
- `crates/dominofish-wasm`: one export for the wasm playout checksum.
- `tools/capi-oracle`: scripts that run Capi's engine as the rules oracle. They read a Capi checkout and never write to it.

[docs/FORMATS.md](docs/FORMATS.md) defines the game records, the engine protocol, and the result files.

## Testing

- Capi parity. The Rust core replays 117,456 golden-trace moves from Capi's engine, 87 edge scenarios, and 200 perft queries, with 0 mismatches. A lockstep fuzzer compares legal moves and full states with Capi's after every move: 0 mismatches over 1,000,000 rounds per mode.
- Never-peeks. Rearranging tiles a seat cannot see, behind the same public record, leaves its information set byte-identical and every bot's move unchanged.
- SPRT. On synthetic data its error rates match alpha and beta (0.05). Of 200 null runs between two copies of one bot, 12 accepted H1, where 10 is the expected count.
- `scripts/check.sh` runs formatting, lint, all tests, the wasm32 build, the report check, and an em dash scan. `scripts/regress.sh` reruns the fast check of every passing feature.

## Limitations

- No human benchmark yet: no result here involves a person.
- Self-play results are relative to these bots.
- 2v2 is a team game, and DominoFish makes no equilibrium claim for it.
- The bots are fixed heuristics; no search engine exists yet.

## Build and run

You need Rust 1.99.1, pinned in `rust-toolchain.toml`.

```bash
cargo build --release
```

```bash
target/release/dominofish selfplay --mode 2v2 --seed 1
```

```bash
scripts/check.sh
```

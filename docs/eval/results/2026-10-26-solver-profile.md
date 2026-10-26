# Solver profile, 2026-10-26

Commit c67e028 (the solver is unchanged since 746d622; Bench 6121081), one thread, Apple M1. Load averages: 5.4 to 5.7 during the samples, 3.2 to 4.2 during the timing runs. Timings are user seconds from `/usr/bin/time -p`.

## Method

BUILD.md's command, `xcrun xctrace record --template 'Time Profiler'`, recorded `dominofish solve-stats` but did not finish after the target exited (0% CPU for 7 minutes; the saved trace failed to export with "Document Missing Template Error"). The profile below comes from `/usr/bin/sample <pid> <seconds> 1` instead (1 ms intervals), on a release build with line tables in its own target directory:

```
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none cargo build --release -p dominofish --target-dir target/profile
target/profile/release/dominofish solve-stats --mode 2v2 --games 120 --seed 1                  # run A, sampled 15 s
target/profile/release/dominofish solve-stats --mode 2v2 --games 200 --seed 1 --table-bits 16  # run B, sampled 10 s
```

## Where the time goes (share of samples by the function on top of the stack)

| Function | Run A: 2^20 entries (16 MB) | Run B: 2^16 entries (1 MB) |
|---|---|---|
| Search::search (with the inlined table probe and store) | 31.5% | 26.4% |
| State::play | 23.9% | 20.4% |
| solver::key | 20.8% | 10.7% |
| solver::ordered | 11.4% | 14.4% |
| State::legal_moves | 9.1% | 20.9% |
| State::undo | 3.3% | 7.2% |
| Samples | 12,104 | 8,116 |

The key function is about 60 instructions with no memory access outside the state; its share halves with the small table, so most of its 20.8% in run A is the stall of the table probe that follows it.

## Table size

`dominofish solve-stats --mode <m> --games 20 --seed 1 --table-bits <b>` (every position of rounds 1 and 2; 1,005 solves in 2v2, 891 in 1v1; 0 capped in every run):

| Table bits | Size | 2v2 user s | 1v1 user s |
|---|---|---|---|
| 12 | 64 KB | 1.48 | 0.38 |
| 14 | 256 KB | 1.40 | 0.23 |
| 16 | 1 MB | 1.32 | 0.17 |
| 17 | 2 MB | 1.35 | 0.17 |
| 18 | 4 MB | 1.40 | 0.18 |
| 20 | 16 MB | 2.51 | 0.27 |

With 2^20 entries the solver spends about half its 2v2 time waiting on table misses; 2^16 entries solve the same positions 1.9 times faster in 2v2 and 1.6 times faster in 1v1.

## Changes tried and reverted

- Loading each child's table entry before searching the first child (Bench unchanged, 6121081): bench 0.56, 0.60, 0.63 s against 0.54, 0.55, 0.53 s; solve-stats 2v2 2.79 s against 2.46 s. Slower.
- `#[inline]` on solver::key and State::position_hash (Bench unchanged): bench 0.54, 0.54, 0.51 s against 0.52, 0.52, 0.53 s; solve-stats with 2^16 entries 1.31 s against 1.32 s. No change.

## Decision

Bench, the omniscient bot, and solve-stats keep the 2^20-entry table: Bench and run R3 are defined with it, and a different table changes node counts. The PIMC engine (M4) uses 2^16 entries by default (docs/PLAN.md, D18). With the small table, move generation and ordering take 35% of the samples and play and undo 28%; they are the next place to look if PIMC needs more worlds per second.

# Playout bench baseline, 2026-10-25

A recorded baseline (M1-10), not a gate and not a strength result. Command: scripts/verify/M1-10.sh (200,000 seeded random rounds per mode, seed 1, one thread, each timing run twice). Commit de3109a. Machine: Apple M1, 8 GB, shared with other work; 1-minute load average 13.94 during the runs, so the speeds are lower bounds and vary between runs. Rerun on an idle machine (load under 2) before comparing speeds.

| Mode | Checksum (native and wasm) | Native rounds/s, run 1 | run 2 | wasm under Node v20.20.0, run 1 | run 2 |
|---|---|---|---|---|---|
| 1v1 | 78c57dbf4108684c | 1,422,987 | 1,433,537 | 1,162,490 | 1,204,983 |
| 2v2 | 74a663b29dbfa9d1 | 1,290,757 | 1,288,643 | 1,045,326 | 933,270 |

Summary line: M1-10: PASS checksums equal native and wasm, 200000 rounds per mode, commit de3109a, 1 thread: [1v1 run 1: checksum 78c57dbf4108684c native 1422987 rounds/s wasm 1162490 rounds/s load 13.94] [1v1 run 2: checksum 78c57dbf4108684c native 1433537 rounds/s wasm 1204983 rounds/s load 13.94] [2v2 run 1: checksum 74a663b29dbfa9d1 native 1290757 rounds/s wasm 1045326 rounds/s load 13.94] [2v2 run 2: checksum 74a663b29dbfa9d1 native 1288643 rounds/s wasm 933270 rounds/s load 13.94]

A round here is one round from a fresh deal on an empty board to its end with uniformly random canonical moves (about 23 plies in 1v1 and 26 in 2v2). The research lead of 1.82M to 2.23M 2v2 rounds per second (C, simplified rules, digest section 4) is not comparable: other rules, other code, other load.

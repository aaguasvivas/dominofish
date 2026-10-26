# Calibration of PIMC's default worlds per move

The rule (docs/PLAN.md D18, committed in 3d52328 before these runs): the default W is the largest of 32, 64, and 128, tried in that order and stopping at the first that fails, for which `dominofish pimc-stats --games 20 --seed 1100000000 --worlds W` gives a p90 time per move of at most 1,000 ms in both modes, in the slower of two runs.

Binary: runs/bin/3d52328/dominofish (commit 3d52328), one thread, nice 10, 2026-10-26. Each `===` line gives the 1-, 5-, and 15-minute load averages and the start time of the run below it.

```
=== W 32 run 1 mode 2v2 load 3.66 3.09 3.15 03:13:03
pimc-stats: mode 2v2 worlds 32 decisions 1116 (forced moves skipped 818); time ms median 4.3 p90 369.0 max 2829.2; capped solves 0; inconsistent 0; fallback 0; wall 184.0 s
=== W 32 run 1 mode 1v1 load 3.33 3.09 3.13 03:16:07
pimc-stats: mode 1v1 worlds 32 decisions 1268 (forced moves skipped 1599); time ms median 3.5 p90 121.9 max 4611.8; capped solves 14; inconsistent 0; fallback 0; wall 63.1 s
=== W 32 run 2 mode 2v2 load 3.24 3.07 3.12 03:17:10
pimc-stats: mode 2v2 worlds 32 decisions 1116 (forced moves skipped 818); time ms median 4.0 p90 372.8 max 2503.9; capped solves 0; inconsistent 0; fallback 0; wall 184.2 s
=== W 32 run 2 mode 1v1 load 4.38 3.38 3.21 03:20:14
pimc-stats: mode 1v1 worlds 32 decisions 1268 (forced moves skipped 1599); time ms median 3.4 p90 121.4 max 4736.9; capped solves 14; inconsistent 0; fallback 0; wall 61.6 s
=== W 64 run 1 mode 2v2 load 2.97 3.15 3.13 03:21:16
pimc-stats: mode 2v2 worlds 64 decisions 1116 (forced moves skipped 818); time ms median 7.8 p90 714.2 max 4955.4; capped solves 0; inconsistent 0; fallback 0; wall 349.1 s
=== W 64 run 1 mode 1v1 load 2.08 2.24 2.67 03:27:05
pimc-stats: mode 1v1 worlds 64 decisions 1268 (forced moves skipped 1599); time ms median 7.0 p90 250.3 max 8524.0; capped solves 27; inconsistent 0; fallback 0; wall 126.6 s
=== W 64 run 2 mode 2v2 load 2.59 2.57 2.75 03:29:11
pimc-stats: mode 2v2 worlds 64 decisions 1116 (forced moves skipped 818); time ms median 7.7 p90 709.5 max 4979.1; capped solves 0; inconsistent 0; fallback 0; wall 348.7 s
=== W 64 run 2 mode 1v1 load 1.78 2.43 2.66 03:35:00
pimc-stats: mode 1v1 worlds 64 decisions 1268 (forced moves skipped 1599); time ms median 6.8 p90 237.7 max 8550.1; capped solves 27; inconsistent 0; fallback 0; wall 123.4 s
=== W 128 run 1 mode 2v2 load 2.38 2.36 2.59 03:37:04
pimc-stats: mode 2v2 worlds 128 decisions 1116 (forced moves skipped 818); time ms median 15.3 p90 1422.1 max 9963.7; capped solves 0; inconsistent 0; fallback 0; wall 697.9 s
=== W 128 run 1 mode 1v1 load 4.92 3.48 2.85 03:48:41
pimc-stats: mode 1v1 worlds 128 decisions 1268 (forced moves skipped 1599); time ms median 14.4 p90 473.5 max 16820.8; capped solves 48; inconsistent 0; fallback 0; wall 244.0 s
=== W 128 run 2 mode 2v2 load 2.92 2.90 2.73 03:52:45
pimc-stats: mode 2v2 worlds 128 decisions 1116 (forced moves skipped 818); time ms median 16.1 p90 1422.8 max 10004.7; capped solves 0; inconsistent 0; fallback 0; wall 700.0 s
=== W 128 run 2 mode 1v1 load 2.24 2.57 2.63 04:04:26
pimc-stats: mode 1v1 worlds 128 decisions 1268 (forced moves skipped 1599); time ms median 14.9 p90 514.1 max 16792.1; capped solves 48; inconsistent 0; fallback 0; wall 256.2 s
W 128 fails
done
```

Result: W = 32 passes (p90 at most 372.8 ms in 2v2 and 121.9 ms in 1v1); W = 64 passes (p90 at most 714.2 ms in 2v2 and 250.3 ms in 1v1); W = 128 fails (p90 1,422.1 and 1,422.8 ms in 2v2). The default is 64 worlds per move (crates/dominofish/src/solve.rs, PIMC_WORLDS). The slowest single moves at 64 worlds took 5.0 s (2v2) and 8.6 s (1v1), so matches with PIMC use --timeout-ms 120,000 as a safety limit.

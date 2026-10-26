# DominoFish audit, 2026-10-25

Scope: the command line, README.md and the documents, the code (two read-only reviewers: code, and documents with their claims), and the new web board. Skill: ship-audit, with its state-and-rules, web-launch, and layout-evidence references. Builds: HEAD 746d622 at the start; fixes in c210628, 06ebc53, f869951, 11534c2, and the documents commit after them. Live: the web board as a private claude.ai artifact, https://claude.ai/artifact/96zYMxRAJySo2pxhUUjMNW, version 2 published from the clean tree at 11534c2 and observed live (the new typeface rendered; a game in progress resumed from browser storage across the republish). The repository has no remote, so nothing else is deployed.

## Verdict

Ready to share as a private project and a private board link; not ready for a public launch, which waits on your decisions on the repository, the license, and publishing (questions Q1 and Q2).

## Fix list

Each item names the build it describes. "Verified" means observed after the fix: in the browser pane for the board, by the command's output for the command line, by a test for the code.

1. **fix, verified (c210628).** State text accepted states no game reaches, and the solver then indexed out of range (a 21-tile hand on an empty board, a locked board in play). The parser now requires a consistent round in play; regression tests hold both of the reviewer's crashing inputs.
2. **fix, verified (c210628).** InfoSet text: a history action `p` with no tile panicked (byte-slice underflow), and a board with ends `--`, a seat of `-`, or 17 tiles on an empty board crashed a bot. The parser checks seat, turn, ends against the board, hand size, and passes; the engine refuses a seat that is not on turn.
3. **fix, verified (06ebc53).** `match --sprt nan,1` and a report file with such bounds panicked. Bounds must be finite with mu0 < mu1.
4. **fix, verified (06ebc53).** `report` panicked on a short pair line and replaced unreadable rounds or wins with 1 and 0; it also printed "load average of inf to -inf" with no loads, and named only the first row's engine seed base for R2, whose rows have one base each. Each is now an error or the true range ("5000100000 to 5000100035 (one per row)").
5. **fix, verified (06ebc53).** Runtime errors (a missing file) exited 2 with the usage text. They now exit 1 without it; usage errors keep exit 2.
6. **fix, verified (06ebc53).** The harness discarded engine stderr, so a crash read "b1: exited". It now reads "b1: exited; stderr: thread 'main' panicked at ..." (test engine runs/dev/panic.sh); a non-UTF-8 line is a protocol fault, not a crash.
7. **fix, verified (HEAD 746d622 plus the working tree; 06ebc53).** Help: the overview was 83 columns wide and "bench-positionsPrint" ran together; three option lists missed --mode; the capi-fuzz help hid its CAPI_DIR and Node defaults. A test now keeps every help line within 80 columns with a gap after each name (widest line now 75).
8. **fix, verified (c210628).** The M3-01 and M3-02 verify scripts were weaker than their feature descriptions: no coverage floor, the fallback and test counts not required, the bench header and depths not checked. Both now check these; both features were re-verified under them.
9. **fix, verified (the documents commit).** Documents: the FORMATS.md match example failed (no --engine-seed); FORMATS.md lacked the state text and the `# run` line; PLAN.md lacked D15 for the solver and listed old commands; README.md said no search engine exists and left Bench out of the check list; Spanish words lacked accents; CLAUDE.md had no bench command. All corrected; FORMATS.md gained State text and Board requests.
10. **fix, verified (11534c2, live).** Board at 320x568: the new-game button covered the end of the wordmark (wordmark right edge 148 px, button left edge 164 px after the fix); in Spanish the partner's seat plate overlapped both rivals' plates (0 overlaps after the fix, measured from the plates' boxes); the right end chip wrapped away from its tile (each chip now sits on its tile's row).
11. **fix, verified (11534c2).** Board at 568x320 landscape: the hand started 139 px below the screen; it now docks at the bottom edge (bar at 266 to 320 px).
12. **fix, verified (11534c2).** Board: every line tile replayed its drop-in animation on each update; only the newly placed tile animates now (one tile with the animation class, measured).
13. **fix, verified (11534c2).** Board copy: the capicúa line read "played the last tile on a tile that fit both ends"; it now reads "went out with a tile that fit both ends, worth 25 more" (Spanish: "dominó con una ficha que entraba por los dos lados, 25 puntos más"). The best line showed names cut to three letters ("Eas", "Wes"); it now shows a team-colored dot with the full name as its label.
14. **fix, verified (11534c2, live).** Board type and palette: Inter and a warm cream ground read as a generic default; the board now uses Archivo, IBM Plex Sans, and IBM Plex Mono on a cool grey tinted toward the logo's navy. The page-thread engine fallback no longer builds code with `new Function`, which a strict content policy can block.
15. **open, low.** The canonical move generator is written twice (rules.rs and infoset.rs) and the SplitMix mixer three times (rng.rs, hash.rs, solver.rs). Both copies are tested; merging them is a refactor with no behavior change, left for a quiet moment.
16. **open, low.** The match header writes Bench as `-` for every engine; it gets a value when an engine searches (M3-04, the omniscient bot).

Checked and clean: the solver's search logic (bound classification, table cuts, the root, the fallback, value range, no allocation; also by naive minimax on every position checked), the em dash sweep (scripts/check.sh fails on U+2014; no U+2013 sentence dashes in README.md or web/src), every README number against REPORT.md or a features.json evidence line, the board's English and Spanish string tables (88 keys each, no mixed-language screen seen in either language), and two full board games driven through the visible controls (122 moves, 21 draws, 9 passes, 5 end choices, 0 errors).

Unmeasured: the board on a real phone (safe-area insets read 0 in the browser pane), keyboard-only play from start to finish, and screen-reader output.

Evidence: docs/audit/2026-10-25/cli-sweep.txt (every command and error path, with exit codes and widths).

## What's left for you

1. Open the board on your phone: https://claude.ai/artifact/96zYMxRAJySo2pxhUUjMNW (private to your account). Play a round in portrait and in landscape, and tell me anything that looks off.
2. To share the board with someone, use the Share menu on that page; until then only you can open it.
3. Answer Q1 and Q2 in docs/PLAN.md (repository and license) before any public launch.

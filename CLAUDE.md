# DominoFish

Before engine work, read the docs/progress.md top block, docs/PLAN.md, and docs/features.json. The full brief is docs/prompts/BUILD.md; each later session follows docs/prompts/CONTINUE.md.

## Commands
- Prefix every rustup, rustc, and cargo command with PATH="/opt/homebrew/opt/rustup/bin:$PATH" (the shell does not read ~/.zshrc).
- Check (must pass on main): scripts/check.sh
- Regression (fast command of every passing feature): scripts/regress.sh
- Selfplay: target/release/dominofish selfplay --mode 2v2 --seed 1 (or --mode 1v1)
- Feature verify: the "verify" command of its docs/features.json entry, from the repo root.
- Parity fuzzer against Capi: target/release/dominofish capi-fuzz --mode 2v2 --rounds 10000 --seed 1 (needs CAPI_DIR with the pinned commit; exit 1 on SKIP or a mismatch)
- Match: scripts/build-bin.sh <commit> prints runs/bin/<commit>/dominofish; then <that binary> match --mode 2v2 --unit round|game --target 100 --pairs N --seed S --engine-seed E --a "<bin> engine --bot X" --b "<bin> engine --bot Y" --out runs/<name>.txt (docs/FORMATS.md, Match results; never target/release)
- Bench: target/release/dominofish bench (last line Bench: <nodes>; check.sh runs it twice)
- Web board: scripts/build-web.sh, then open web/dist/index.html; verify scripts/verify/M11-01.sh
- Git: /usr/bin/git only. Disk: df -g /Users/Adelson before builds and long runs.

## Hard constraints (BUILD.md numbers)
1. Information sets only: bots and the public API see only the InfoSet for their seat; no type reachable from InfoSet holds hidden tiles; the omniscient bot is labeled as cheating.
3. Capi is read-only: never write, install, build, or commit inside /Users/Adelson/Desktop/personal/capi; oracle tools read it through CAPI_DIR.
5. No test gaming: never loosen a test, invariant, fuzz count, bound, sample size, or confidence level, never change seeds after a result; commit run settings in PLAN.md before a run; tune on dev seeds, claim on held-out seeds.
6. An illegal move, crash, timeout, or inconsistent, fallback, or biased sampler world voids every strength result of that run.
10. Long runs: nohup nice -n 10 <command> > runs/<name>.log 2>&1 & from the main session, at most 4 threads, PID and ETA in the progress top block; peak RSS under 2 GB per process and 3 GB total; below 8 GB free run cargo clean, below 5 GB stop and ask.
12. Small commits straight to main, feature id first, identity aaguasvivas <adelsonaguasvivas@gmail.com>, no Co-Authored-By trailer; from M3 on "Bench: <nodes>" last on search or eval commits; never force-push, rewrite history, or reset over uncommitted work.
13. No em dashes anywhere (check.sh fails on U+2014); literal wording; no employer names or identifiers; files for the owner go under docs/, named by absolute path.

## Working rules
- One feature at a time: the first failing feature in milestone order (then id) whose requires pass. Only passes and evidence change in features.json, and passes turns true only after its verify exits 0 in this session.
- Ask first, in one batched message: rule changes or re-pinning Capi, durable identifiers, licenses, remotes, pushing, copied code, copyleft or heavy dependencies, ML frameworks, git LFS, large binaries, cloud or paid services, persistent config outside the repo or under .claude/, changing a pre-registered run, editing a committed result, runs over 6 hours, anything out of scope.
- When I ask what is next or what is left, answer with a short numbered list split into what you can do now (then do it) and what only I can do (decisions, accounts, purchases, runs on my time).
- When compacting, keep the current feature id, files changed, exact commands with seeds, the last Bench number, open failures with seeds, running jobs with log paths, and every owner decision word for word.

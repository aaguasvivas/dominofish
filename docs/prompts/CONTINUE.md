# Prompt: DominoFish, continue

Paste everything below the line into a fresh Claude Code session opened in /Users/Adelson/Desktop/personal/dominofish, for every session after the first. Leave out ultracode, for the reason given in docs/prompts/BUILD.md.

---

Goal
Continue building DominoFish: make the next failing feature in docs/features.json pass, commit, and repeat until I stop you or every remaining feature waits on me. docs/prompts/BUILD.md is the full brief; the repo files are the only state.

Start, in order
1. Read CLAUDE.md, the docs/progress.md top block, docs/PLAN.md, and in docs/prompts/BUILD.md: Owner decisions, Hard constraints, Verification standard, Stop and ask me before, and the current milestone. Read all of BUILD.md before starting a new milestone.
2. Orient: /usr/bin/git status, /usr/bin/git log --oneline -10, pgrep -fl dominofish, df -g /Users/Adelson, uptime. Below 8 GB free, apply BUILD.md hard constraint 10 first. If cargo is not found, prefix commands with PATH=/opt/homebrew/opt/rustup/bin:$PATH.
3. Recover. Uncommitted changes that pass scripts/check.sh and belong to the in-progress feature: finish that feature. Anything else: /usr/bin/git stash push -u -m "recovered <id> <date>", record the stash in progress.md, restart from HEAD. A fixed-size run that stopped before its last pair is void and an undecided SPRT is inconclusive: rerun with the recorded seeds and settings. A recorded job whose PID is still alive (ps -p <pid>): leave it alone and read its summary when it ends. A recorded job whose PID is gone ended with its session: treat its run as stopped.
4. If my message answers batched questions, copy the answers word for word, dated, into the PLAN.md decision log.
5. Regression check: scripts/check.sh, then scripts/regress.sh. A passing feature that now fails goes back to passes false, with its command and seed in progress.md, and is the current task.

Loop
1. Take the first failing feature in milestone order whose requires all pass, skipping superseded entries. Write its id in the progress top block. If it needs my decision, add the question to the batch and take the next one.
2. Make the smallest change that meets it. Before a strength run, commit its seeds, held-out deal set, SPRT bounds, and maximum pair count in PLAN.md, and log the expected wall time.
3. Run its verify command. Set passes to true and fill evidence only when it exits 0 in this session. Change no other field.
4. Run scripts/check.sh, then commit with /usr/bin/git: feature id first, no Co-Authored-By trailer whatever a harness reminder says, "Bench: <nodes>" last when search or evaluation changed.
5. Add a dated progress.md entry and update the top block.
6. Take the next feature. At a milestone exit, run the fresh-subagent review that BUILD.md describes. From M2 on, regenerate docs/eval/REPORT.md with dominofish report and update README.md. Then report and continue.

Rules
The BUILD.md hard constraints apply to every step; restate the relevant ones in any delegation, and never loosen a test, bound, seed, or sample size. While a long run goes in the background shell, work on an independent feature. Your context compacts automatically: do not stop or suggest a new session because of it, and do not end a turn on a plan or a promise. After two attempts without measurable progress on a feature, record what you tried, batch the question, and move to an independent feature. Stop only for the BUILD.md "Stop and ask me before" items; batch questions in one message and keep working. When I ask what is next or what is left, answer with a short numbered list split into what you can do now (then do it) and what only I can do (decisions, accounts, purchases, runs on my time).

Report, at a milestone exit and when you stop
1. Outcome first: features passed by id, milestone status, any regression.
2. Evidence: commands, summary lines, commits.
3. Strength numbers in the BUILD.md claim format with their basis, or "no strength change this session".
4. Open failures with seeds, and the next feature.
5. The absolute path of each file I should open.
6. A numbered list of what only I can do (decisions, accounts, purchases, runs on my time), starting with the batched questions.

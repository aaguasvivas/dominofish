#!/usr/bin/env bash
# M2-07: SPRT (alpha = beta = 0.05) on per-pair results and 95% CIs for
# points per round and game-win rate. Checks: the pre-registered synthetic
# test T1 (docs/PLAN.md): error rates within 0.05 +- 3 SE in both
# configurations and under 1% of runs at the maximum; the summary and SPRT
# formulas; and, with HEAD's runs/bin binary, a match with --sprt that stops
# at a decision (identical bots accept H0 at 16 pairs) and a full-game match
# whose summary carries both CIs (identical bots in both, so no strength is
# measured outside a pre-registered run). --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M2-07
fail=0
err() { echo "ERROR $*"; fail=1; }
log=runs/M2-07/tests.log
cargo test -p dominofish --bin dominofish stats -- --nocapture > "$log" 2>&1 || err "stats tests failed ($log)"
for t in t1_sprt_error_rates_match_alpha_and_beta summary_and_interval_match_the_formulas sprt_bounds_and_simple_decisions; do
  grep -q "test stats::tests::$t ... ok" "$log" || err "test $t did not pass"
done
t1=$(grep -o 'sprt t1: .*' "$log")
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M2-07: FAIL"; exit 1; }
out=$("$bin" match --mode 2v2 --unit round --target 100 --pairs 500 --seed 8000 --engine-seed 5000000070 --sprt 0,4 \
  --a "$bin engine --bot heaviest" --b "$bin engine --bot heaviest" --out runs/M2-07/null-sprt.txt)
case "$out" in *"H0 after 16 pairs"*"match: PASS"*) ;; *) err "null SPRT: $out" ;; esac
out=$("$bin" match --mode 1v1 --unit game --target 100 --pairs 10 --seed 8100 --engine-seed 5000000071 \
  --a "$bin engine --bot rule" --b "$bin engine --bot rule" --out runs/M2-07/game-ci.txt)
case "$out" in *"points per round"*"95% CI"*"game-win rate of A"*"95% CI"*"match: PASS"*) ;; *) err "game CIs: $out" ;; esac
grep -q '^# summary points per round' runs/M2-07/game-ci.txt || err "no summary line in the results file"
if [ "$fail" -eq 0 ]; then
  echo "M2-07: PASS ${t1#sprt t1: }; null match stops at H0 after 16 pairs; points-per-round and game-win CIs reported"
else
  echo "M2-07: FAIL"
fi
exit "$fail"

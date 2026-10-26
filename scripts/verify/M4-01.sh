#!/usr/bin/env bash
# M4-01: the world sampler (core/src/sampler.rs; docs/PLAN.md D17), by test
# T3 (docs/PLAN.md, Pre-registered runs), in release mode:
# (a) chi-square against exact enumeration on 100 2v2, 100 1v1, and 30 1v1
#     no-double decisions with 2 to 2,000 consistent worlds: at most 5 (2
#     for no-double) decisions with p < 0.01 and a pooled p of at least
#     0.001 per set;
# (b) the true world at the uniform rate: |z| <= 3.29 per mode, expected
#     draws at least 200;
# (c) every sampled world replays the round: 0 inconsistent worlds;
# and the sampler counts equal the M2-03 counts, the chi-square p-values
# match known quantiles, and the allocation test reports 0 allocations with
# sampled worlds counted. --fast runs the smoke sizes (a few games), which
# check (c) and the counts but do not judge (a) and (b).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M4-01
fail=0
err() { echo "ERROR $*"; fail=1; }
full=1
[ "${1:-}" = "--fast" ] && full=0
log=runs/M4-01/sampler.log
T3_FULL=$full cargo test -q --release -p dominofish-core --test sampler -- --nocapture --test-threads=1 > "$log" 2>&1 || err "sampler tests failed ($log)"
grep -q 'test result: ok. 4 passed' "$log" || err "the sampler tests did not all pass ($log)"
uniform=$(grep -o 'sampler uniformity .*' "$log")
truth=$(grep -o 'sampler truth .*' "$log")
counts=$(grep -o 'sampler counts: .*' "$log")
[ -n "$uniform" ] && [ -n "$truth" ] && [ -n "$counts" ] || err "missing summary lines ($log)"
case "$uniform$truth" in *"inconsistent "[1-9]*) err "inconsistent worlds: $uniform $truth" ;; esac
if [ "$full" -eq 1 ]; then
  case "$uniform" in *"T3 (a) as registered"*) ;; *) err "T3 (a) did not run at its registered size" ;; esac
  case "$truth" in *"T3 (b) as registered"*) ;; *) err "T3 (b) did not run at its registered size" ;; esac
  for set in "2v2:100:5" "1v1:100:5" "1v1 no-double:30:2"; do
    name=${set%%:*}; rest=${set#*:}; want=${rest%%:*}; max_low=${rest#*:}
    part=$(printf '%s' "$uniform" | grep -o "\[$name: [^]]*\]")
    n=$(printf '%s' "$part" | sed -n 's/.*decisions \([0-9]*\) .*/\1/p')
    low=$(printf '%s' "$part" | sed -n 's/.*p<0.01 in \([0-9]*\) .*/\1/p')
    pooled=$(printf '%s' "$part" | sed -n 's/.* p \([0-9.]*\) inconsistent.*/\1/p')
    [ "${n:-0}" -eq "$want" ] || err "$name: $n decisions (target $want)"
    [ "${low:-99}" -le "$max_low" ] || err "$name: $low decisions with p < 0.01 (target at most $max_low)"
    awk -v p="${pooled:-0}" 'BEGIN { exit !(p >= 0.001) }' || err "$name: pooled p $pooled (target at least 0.001)"
  done
  for mode in TwoVsTwo OneVsOne; do
    part=$(printf '%s' "$truth" | grep -o "\[$mode: [^]]*\]")
    z=$(printf '%s' "$part" | sed -n 's/.* z \([-0-9.]*\) .*/\1/p')
    e=$(printf '%s' "$part" | sed -n 's/.*expected \([0-9.]*\) .*/\1/p')
    awk -v z="${z:-99}" 'BEGIN { exit !(z <= 3.29 && z >= -3.29) }' || err "$mode: z $z (target |z| <= 3.29)"
    awk -v e="${e:-0}" 'BEGIN { exit !(e >= 200) }' || err "$mode: expected true-world draws $e (target at least 200)"
  done
fi
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in
  *"sampled worlds "[1-9]*"allocations 0 (hash check true)") ;;
  *) err "allocation test: ${alloc:-no summary}" ;;
esac
worlds=$(printf '%s' "$alloc" | sed -n 's/.*sampled worlds \([0-9]*\) .*/\1/p')
if [ "$fail" -eq 0 ]; then
  echo "M4-01: PASS ${uniform#sampler }; ${truth#sampler }; ${counts#sampler }; 0 allocations over $worlds sampled worlds"
else
  echo "M4-01: FAIL"
fi
exit "$fail"

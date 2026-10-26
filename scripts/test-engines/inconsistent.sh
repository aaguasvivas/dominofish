#!/usr/bin/env bash
# A faulty engine for the M4-03 harness check (inconsistent): it plays the
# moves of the engine it wraps, but reports one sampled world
# that disagrees with its InfoSet before its first move, as a broken PIMC
# sampler would; the referee must stop the match with a Sampler fault.
# Usage: inconsistent.sh <engine command...> (the engine that chooses moves).
first=1
"$@" | while IFS= read -r line; do
  case "$line" in
    move*)
      if [ "$first" -eq 1 ]; then
        echo "info inconsistent"
        first=0
      fi
      ;;
  esac
  echo "$line"
done

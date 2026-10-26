#!/usr/bin/env bash
# A faulty engine for the M2-06 harness checks (stray): it writes a move
# line right after its header echo, out of turn. The referee must reject it
# before this engine's first go instead of taking it as that move.
while IFS= read -r line; do
  case "$line" in
    "dominofish 1") echo "dominofish 1 ready budgets=- threads=1 tables=- view=infoset" ;;
    header*) printf '%s\nmove pass\n' "$line" ;;
    go) echo "move pass" ;;
    quit) exit 0 ;;
  esac
done

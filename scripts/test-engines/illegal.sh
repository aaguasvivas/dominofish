#!/usr/bin/env bash
# A faulty engine for the M2-06 harness checks (illegal): it answers the
# handshake and then misbehaves.
while IFS= read -r line; do
  case "$line" in
    "dominofish 1") echo "dominofish 1 ready budgets=- threads=1 tables=- view=infoset" ;;
    header*) echo "$line" ;;
    go) echo "move draw" ;;
    quit) exit 0 ;;
  esac
done

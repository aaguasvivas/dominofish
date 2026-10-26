#!/usr/bin/env bash
# A faulty engine for the M2-06 harness checks (pass): it answers the
# handshake and then passes on every turn, which is illegal while it holds
# a fitting tile or, in 1v1, while the boneyard has tiles.
while IFS= read -r line; do
  case "$line" in
    "dominofish 1") echo "dominofish 1 ready budgets=- threads=1 tables=- view=infoset" ;;
    header*) echo "$line" ;;
    go) echo "move pass" ;;
    quit) exit 0 ;;
  esac
done

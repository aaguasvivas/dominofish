#!/usr/bin/env bash
# A faulty engine for the M2-06 harness checks (flood): it answers the
# handshake and then, on its first go, writes info lines every 50 ms and
# never moves. The per-move deadline must still end the match.
while IFS= read -r line; do
  case "$line" in
    "dominofish 1") echo "dominofish 1 ready budgets=- threads=1 tables=-" ;;
    header*) echo "$line" ;;
    go) while :; do echo "info thinking"; sleep 0.05; done ;;
    quit) exit 0 ;;
  esac
done

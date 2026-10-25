#!/usr/bin/env bash
# A faulty engine for the M2-06 harness checks (silent): it answers the
# handshake and then misbehaves.
while IFS= read -r line; do
  case "$line" in
    "dominofish 1") echo "dominofish 1 ready" ;;
    header*) echo "$line" ;;
    go) exec sleep 600 ;;
    quit) exit 0 ;;
  esac
done

#!/usr/bin/env bash
# A wrapper for the M2-06 harness checks (spy): spy.sh <log> <engine
# command...> runs the engine and appends every line the referee sends it
# to <log>, so the check can read what an engine receives.
log=$1
shift
exec "$@" < <(tee -a "$log")

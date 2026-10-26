#!/usr/bin/env bash
# A wrapper for the M2-06 harness checks (spy): spy.sh <log> <engine
# command...> runs the engine and appends every line the referee sends it
# to <log>, so the check can read what an engine receives. A %p in <log>
# becomes the wrapper's process id, so the two engines of a 2v2 side write
# separate files.
log=${1//%p/$$}
shift
exec "$@" < <(tee -a "$log")

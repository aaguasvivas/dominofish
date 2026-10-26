#!/usr/bin/env bash
# A wrapper for the M4-07 memory check: rss.sh <log> <engine command...>
# runs the engine under /usr/bin/time -l and writes its report, with the
# peak resident set size, to <log> with the wrapper's process id appended,
# so each engine of a side writes its own file.
log=$1
shift
/usr/bin/time -l "$@" 2> "$log.$$"

# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# bounded.sh — a foreign tool gets a clock.
#
# Sourced, not executed.
#
# ## Why this exists
#
# Measured 2026-09-06 on the Linux test host, in the gate cell `results-record`:
# Cyclone DDS `idlc` on the ten-line corpus case
# `recursive-optional-external` — a `@mutable struct Node` whose only
# other member is `@optional @external Node next` — ran for 21 minutes
# of CPU at 100% and held 8.7 GB of resident memory, growing about
# 26 MB every 30 seconds. It does not terminate on that file; it
# allocates.
#
# Nothing bounded it. The cell had been running for 71 minutes with one
# line of output, and the host had 7 GB left of 15.
#
# What that cost is already in the ledger. Every `recursive-optional-external`
# row for Cyclone reads `refused — idlc failed and named no case`, which
# is what a process looks like after the kernel takes it for memory: it
# exits non-zero and says nothing. The row records that as an answer
# about the case. It is an answer about this harness.
#
# ## What it bounds
#
# The wall clock for every front end, and the address space for the
# native ones.
#
# The clock alone is not enough, measured on the next case in the same
# run: `idlc` on `recursive-typedef-sequence` reached 8.8 GB in five
# minutes — about 1.7 GB a minute, sixty times the rate of the case
# before it. A 300-second clock would let that one take 8 GB of the
# host's 15 before it fired. The rate is a property of the case, not of
# the tool, so no single clock is the right bound on its own.
#
# The ceiling is `ulimit -v`, which counts *reserved* virtual space.
# That is why it is not applied to every front end: a JVM reserves far
# more than it uses, so a limit low enough to catch a runaway native
# compiler refuses a Java one before it starts — a second silent refusal
# in place of the first. `bounded_run` is the clock alone;
# `bounded_run_native` adds the ceiling.
#
# Which front end is which was measured, not assumed, and the first
# assumption was wrong: `fastddsgen` got the ceiling and answered with a
# JVM crash report (`hs_err_pid…log`) instead of generating anything.
# `file(1)` on each of the seven, on the Linux test host:
#
#   idlc          ELF executable          ceiling
#   tao_idl       ELF executable          ceiling
#   opendds_idl   ELF executable          ceiling
#   omniidl       Python script           ceiling
#   fastddsgen    shell script -> java    clock alone
#   rtiddsgen     shell script -> java    clock alone
#   jacorb idl    shell script -> java    clock alone
#
# `opendds_idl` carries the string `java` and is an ELF all the same; it
# was run under the ceiling to settle it, and reached its own template
# lookup rather than an allocation failure. `omniidl` is Python, which
# does not over-reserve, and answered under the ceiling.
#
# A native tool that hits the ceiling fails its own allocation and says
# so in its own words, which is a better answer than the kernel taking
# it: the diagnostic reaches the log either way.
#
# ## The reason it writes
#
# `timeout` exits 124 when it fires. That becomes a sentence naming the
# tool, the bound and the fact that the tool said nothing — so a reader
# of `ledger.tsv` can tell "this front end refuses the construct" from
# "this front end did not come back", which are different findings and
# read identically today.

# Seconds a foreign front end gets before it is taken to have hung.
# Generous on purpose: the slowest front end in the corpus answers well
# inside it, so a run that hits this bound has not answered slowly, it
# has not answered.
WIREPROBE_TOOL_SECONDS="${WIREPROBE_TOOL_SECONDS:-300}"

# Kilobytes of address space a *native* front end may reserve. Four
# gigabytes: far above what any front end in the corpus needs to answer,
# far below the fifteen the measurement host has, so a runaway is
# stopped while every real answer still fits.
WIREPROBE_TOOL_KILOBYTES="${WIREPROBE_TOOL_KILOBYTES:-4194304}"

# Run one foreign tool under the clock, and optionally a ceiling.
#
#   bounded_run        "jacorb idl" "$JACORB/bin/idl" -d . gen.idl > idl.log 2>&1
#   bounded_run_native idlc         "$CYCLONE/bin/idlc" gen.idl     > idlc.log 2>&1
#
# Returns the tool's own exit status, except that a tool the clock
# stopped returns 124 (or 137, where it ignored TERM) and has had the
# reason written to stderr.
#
# No redirection inside. The caller writes the tool's own output to a log
# it then quotes — `> idlc.log 2>&1` around the whole call — and a
# `2>/dev/null` here would override it and discard every front end's
# diagnostic, which is the failure this file exists to stop rather than
# repeat.
_bounded_run() {
    local ceiling="$1"
    local what="$2"
    shift 2
    local status=0
    (
        # TERM at the deadline and KILL ten seconds later, which is what
        # yields a reliable 124. Measured 2026-09-06: `--signal=KILL`
        # alone returns 137 through the shell instead, and the reason
        # below was never written — the first run of this file's own
        # test found it.
        [ "$ceiling" = "yes" ] && { ulimit -v "$WIREPROBE_TOOL_KILOBYTES" 2>/dev/null || true; }
        "$WIREPROBE_TIMEOUT" --kill-after=10 "$WIREPROBE_TOOL_SECONDS" "$@"
    ) || status=$?
    if [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
        echo "wireprobe: $what did not return within ${WIREPROBE_TOOL_SECONDS}s and was stopped;" \
             "it produced no diagnostic, so this is not a refusal of the construct" >&2
    elif [ "$ceiling" = "yes" ] && [ "$status" -ne 0 ]; then
        # The bounds, named on every failure of a bounded tool.
        #
        # A tool that hits the ceiling fails its own allocation and says
        # something generic — Cyclone `idlc` on
        # `recursive-typedef-sequence` says `Failed to compile 'gen.idl'`,
        # measured, which reads exactly like a refusal of the construct.
        # Telling the two apart from the exit status alone is not
        # possible, and measuring peak memory would need a `time(1)` the
        # measurement host does not have. So the bounds that were in
        # force are stated rather than guessed at: a reader who sees a
        # real diagnostic ignores this line, a reader who sees a bare
        # failure learns the run was not unbounded.
        echo "wireprobe: $what ran under a ${WIREPROBE_TOOL_KILOBYTES}K ceiling" \
             "and a ${WIREPROBE_TOOL_SECONDS}s clock" >&2
    fi
    return "$status"
}

# The clock alone, for a JVM front end.
bounded_run() { _bounded_run no "$@"; }

# The clock and the ceiling, for a native one.
bounded_run_native() { _bounded_run yes "$@"; }

# `timeout` is coreutils. It is `gtimeout` where coreutils is installed
# beside a BSD userland, which is the same program under the name that
# does not collide — taken so that a driver can be exercised on a
# developer's machine and not only on the measurement host.
#
# Absent entirely, this is a missing package and not a platform: said
# once, loudly, rather than degrading to the unbounded behaviour this
# file exists to end.
if command -v timeout > /dev/null 2>&1; then
    WIREPROBE_TIMEOUT=timeout
elif command -v gtimeout > /dev/null 2>&1; then
    WIREPROBE_TIMEOUT=gtimeout
else
    echo "wireprobe: \`timeout\` (coreutils, or \`gtimeout\`) is required to bound a" \
         "foreign front end" >&2
    exit 1
fi

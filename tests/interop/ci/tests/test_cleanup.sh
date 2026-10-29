#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Verifies the exact-PID cleanup contract of the #28 interop runner: a failed
# cell removes ONLY the children it started, never an unrelated process.
# Pure shell, no DDS stack — runnable on any host including macOS.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=tests/interop/ci/lib.sh
. "$HERE/../lib.sh"

fail() { echo "FAIL: $1" >&2; exit 1; }

# An unrelated "bystander" process the runner must never touch.
sleep 30 &
BYSTANDER=$!

# Two tracked children (as a cell would start).
sleep 30 &
track $!
CHILD_A=$!
sleep 30 &
track $!
CHILD_B=$!

# Trigger the same cleanup the runner's EXIT trap uses.
cleanup

# Give the kernel a moment to reap.
sleep 0.3

kill -0 "$CHILD_A" 2>/dev/null && fail "tracked child A ($CHILD_A) survived cleanup"
kill -0 "$CHILD_B" 2>/dev/null && fail "tracked child B ($CHILD_B) survived cleanup"
kill -0 "$BYSTANDER" 2>/dev/null || fail "bystander ($BYSTANDER) was killed — cleanup was not exact-PID"

# Clean up the bystander ourselves (exact PID, of course).
kill -9 "$BYSTANDER" 2>/dev/null || true
wait "$BYSTANDER" 2>/dev/null || true

echo "PASS: cleanup killed only the tracked children, spared the bystander"

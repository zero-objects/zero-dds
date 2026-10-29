#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Shared helpers for the #28 interop gate runner. Kept separate so the
# exact-PID cleanup contract can be unit tested without any DDS stack.

# Exact-PID process tracking. The gate NEVER uses a broad `pkill` or a
# container-name wildcard — it kills only the specific child PIDs it started,
# so a concurrent unrelated process on the runner is never touched.
CHILD_PIDS=()

# track <pid>: register a child PID for cleanup.
track() { CHILD_PIDS+=("$1"); }

# kill_pid <pid>: terminate exactly this PID and reap it. No-op on empty.
kill_pid() {
  local p="${1:-}"
  [ -n "$p" ] || return 0
  kill -9 "$p" 2>/dev/null || true
  wait "$p" 2>/dev/null || true
}

# cleanup: kill every tracked child PID, nothing else.
cleanup() {
  local p
  for p in "${CHILD_PIDS[@]:-}"; do
    kill_pid "$p"
  done
}

#!/usr/bin/env bash
# L1 Isolation: same user, two processes on the same host.
# Usage: ./host/l1.sh <udp|shm|uds|uds-abstract>
set -euo pipefail

transport="${1:-udp}"
count="${COUNT:-20}"
repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
bin="$repo_root/target/release/isolation-smoke"

# Shared cleanup (phase2-0-smells #12): tmp-Dirs + Background-PIDs
# raeumen auch bei Script-Fehler oder SIGINT. Ohne trap hing `rm -rf`
# am Happy-Path und lief bei Fehler nicht; Test-Runs unter `set -e`
# lassen sonst gelecktes `/tmp/tmp.XXXX` + stranded rx/tx-Prozesse zurueck.
_cleanup_pids=()
_cleanup_dirs=()
cleanup() {
  local pid
  for pid in "${_cleanup_pids[@]:-}"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  local dir
  for dir in "${_cleanup_dirs[@]:-}"; do
    [[ -n "$dir" && -d "$dir" ]] && rm -rf "$dir"
  done
}
trap cleanup EXIT INT TERM

# Build once if missing or stale. `cargo build --release -p
# dds-isolation-smoke` is idempotent when sources are unchanged.
(cd "$repo_root" && cargo build -q --release -p dds-isolation-smoke)

case "$transport" in
  udp)
    "$bin" --transport=udp --role=receiver \
           --local=127.0.0.1:17400 --count="$count" &
    rx=$!
    _cleanup_pids+=("$rx")
    sleep 0.3
    "$bin" --transport=udp --role=sender \
           --local=127.0.0.1:17401 --peer=127.0.0.1:17400 --count="$count"
    wait "$rx"
    ;;
  shm)
    tmp="$(mktemp -d)"
    _cleanup_dirs+=("$tmp")
    # Sender/owner runs first to create the segment, then sleeps;
    # receiver polls until the segment is visible.
    "$bin" --transport=shm --role=sender \
           --local-id=01 --peer-id=02 --base-dir="$tmp" --count="$count" &
    tx=$!
    _cleanup_pids+=("$tx")
    "$bin" --transport=shm --role=receiver \
           --local-id=02 --peer-id=01 --base-dir="$tmp" --count="$count"
    wait "$tx"
    ;;
  uds)
    tmp="$(mktemp -d)"
    _cleanup_dirs+=("$tmp")
    "$bin" --transport=uds --role=receiver \
           --local-id=10 --base-dir="$tmp" --count="$count" &
    rx=$!
    _cleanup_pids+=("$rx")
    sleep 0.3
    "$bin" --transport=uds --role=sender \
           --local-id=11 --peer-id=10 --base-dir="$tmp" --count="$count"
    wait "$rx"
    ;;
  uds-abstract)
    if [[ "$(uname)" != "Linux" ]]; then
      echo "uds-abstract is Linux-only; skipping" >&2
      exit 0
    fi
    prefix="zerodds-l1-$$"
    "$bin" --transport=uds-abstract --role=receiver \
           --local-id=20 --abstract-prefix="$prefix" --count="$count" &
    rx=$!
    _cleanup_pids+=("$rx")
    sleep 0.3
    "$bin" --transport=uds-abstract --role=sender \
           --local-id=21 --peer-id=20 --abstract-prefix="$prefix" --count="$count"
    wait "$rx"
    ;;
  *)
    echo "unknown transport: $transport" >&2
    exit 2
    ;;
esac

echo "L1 smoke ok for $transport (count=$count)"

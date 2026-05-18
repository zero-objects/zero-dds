#!/usr/bin/env bash
# L2 Isolation: sender and receiver as different Unix users.
# Usage: sudo ./host/l2_different_user.sh <udp|shm|uds|uds-abstract>
#
# Requirements:
#   - sudo access (script invokes `sudo -u`)
#   - a user named `zerodds-peer` must exist (create with:
#       sudo useradd -m -s /bin/bash zerodds-peer)
#
# The base_dir / socket paths must be reachable by both users —
# /tmp with 1777 works out of the box on Linux + macOS.
set -euo pipefail

# Shared cleanup (phase2-0-smells #12).
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

transport="${1:-uds}"
count="${COUNT:-20}"
peer_user="${ZERODDS_PEER_USER:-zerodds-peer}"

if ! id -u "$peer_user" >/dev/null 2>&1; then
  echo "peer user '$peer_user' does not exist; create with:" >&2
  echo "    sudo useradd -m -s /bin/bash $peer_user" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
bin="$repo_root/target/release/isolation-smoke"
(cd "$repo_root" && cargo build -q --release -p dds-isolation-smoke)

case "$transport" in
  udp)
    sudo -u "$peer_user" "$bin" --transport=udp --role=receiver \
                                 --local=127.0.0.1:17500 --count="$count" &
    rx=$!
    _cleanup_pids+=("$rx")
    sleep 0.3
    "$bin" --transport=udp --role=sender \
           --local=127.0.0.1:17501 --peer=127.0.0.1:17500 --count="$count"
    wait "$rx"
    ;;
  uds|shm)
    tmp="$(mktemp -d)"
    _cleanup_dirs+=("$tmp")
    chmod 0777 "$tmp"  # both users must be able to bind/mmap
    # Sender/owner creates the resource first (receiver polls).
    if [[ "$transport" = "shm" ]]; then
      sudo -u "$peer_user" "$bin" --transport=shm --role=sender \
             --local-id=30 --peer-id=31 --base-dir="$tmp" --count="$count" &
      tx=$!
      _cleanup_pids+=("$tx")
      "$bin" --transport=shm --role=receiver \
             --local-id=31 --peer-id=30 --base-dir="$tmp" --count="$count"
      wait "$tx"
    else
      sudo -u "$peer_user" "$bin" --transport=uds --role=receiver \
             --local-id=40 --base-dir="$tmp" --count="$count" &
      rx=$!
      _cleanup_pids+=("$rx")
      sleep 0.3
      "$bin" --transport=uds --role=sender \
             --local-id=41 --peer-id=40 --base-dir="$tmp" --count="$count"
      wait "$rx"
    fi
    # tmp wird durch cleanup-trap entfernt
    ;;
  uds-abstract)
    # Abstract-namespace sockets are per-network-namespace. Different
    # Unix users in the same net-ns see each other — this test
    # confirms that Abstract is NOT isolated by user (which is the
    # intended behavior: container-IPC-friendly).
    prefix="zerodds-l2-$$"
    sudo -u "$peer_user" "$bin" --transport=uds-abstract --role=receiver \
           --local-id=50 --abstract-prefix="$prefix" --count="$count" &
    rx=$!
    _cleanup_pids+=("$rx")
    sleep 0.3
    "$bin" --transport=uds-abstract --role=sender \
           --local-id=51 --peer-id=50 --abstract-prefix="$prefix" --count="$count"
    wait "$rx"
    ;;
  *)
    echo "unknown transport: $transport" >&2
    exit 2
    ;;
esac

echo "L2 smoke ok for $transport (user $(whoami) ↔ $peer_user, count=$count)"

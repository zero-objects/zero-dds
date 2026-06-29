#!/usr/bin/env bash
# L4 Isolation: Sender und Receiver auf verschiedenen Hosts im LAN.
# ZeroDDS-Bench-Setup: llvm (bare-metal) und pivot (LXC), beide
# in 192.168.178.0/24 (siehe internal/plans/milestone-v1.2.md §Bench-
# Hardware).
#
# Usage:
#   ./l4/cross_host.sh <udp|shm-not-applicable|uds-not-applicable>
#
# SHM + UDS sind by design intra-host only — fuer L4 gibt es nur
# UDP (und spaeter TCP). Das Script schickt die Smoke-Binary via
# scp an den Remote-Host, startet den Receiver dort, und lokal
# den Sender.
set -euo pipefail

transport="${1:-udp}"
count="${COUNT:-50}"
rx_host="${ZERODDS_RX_HOST:-llvm@llvm}"
tx_host="${ZERODDS_TX_HOST:-root@pivot}"
rx_port="${ZERODDS_RX_PORT:-17400}"
tx_port="${ZERODDS_TX_PORT:-17401}"

case "$transport" in
  udp|tcp)
    ;;
  shm|uds|uds-abstract)
    echo "$transport is intra-host only — L4 is not applicable." >&2
    exit 0
    ;;
  *)
    echo "unknown transport: $transport" >&2
    exit 2
    ;;
esac

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
bin="$repo_root/target/release/isolation-smoke"

echo "building smoke binary locally..."
(cd "$repo_root" && cargo build -q --release -p dds-isolation-smoke)

echo "shipping binary to rx host $rx_host..."
# If rx_host has its own build of the binary, rebuild there instead —
# cross-compile between macOS dev and Linux peer is painful. For the
# ZeroDDS bench-setup (llvm is Debian, we cross-build), we ship the
# sources via tar and rebuild on the remote.
#
# Minimal impl: rebuild remotely. rsync-based delta-ship is a
# WP 2.3 harness optimization.
tar -cf - --exclude-vcs --exclude=target -C "$repo_root" . \
  | ssh "$rx_host" \
        "mkdir -p ~/zerodds-smoke && cd ~/zerodds-smoke && tar xf - && source ~/.cargo/env && cargo build -q --release -p dds-isolation-smoke"

rx_host_only="${rx_host#*@}"
# Resolve the RX host's LAN IP. ssh-config alias → getent.
rx_ip="$(ssh "$rx_host" "hostname -I | awk '{print \$1}'")"
echo "rx_ip=$rx_ip"

echo "starting receiver on $rx_host..."
ssh "$rx_host" "source ~/.cargo/env && ~/zerodds-smoke/target/release/isolation-smoke \
     --transport=$transport --role=receiver \
     --local=0.0.0.0:$rx_port --count=$count" &
rx_pid=$!

sleep 1.0
echo "starting sender locally..."
"$bin" --transport="$transport" --role=sender \
       --local=0.0.0.0:"$tx_port" --peer="$rx_ip:$rx_port" --count="$count"

wait "$rx_pid"
echo "L4 smoke ok for $transport via $rx_host (count=$count)"

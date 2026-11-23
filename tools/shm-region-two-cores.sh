#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# shm-region-two-cores — zerodds-shm-transport-1.0 §6.4 without an
# operating system: builds crates/transport-shm/bare-metal/two-cores for
# aarch64-unknown-none and runs it on two Cortex-A53 cores of QEMU's `virt`
# machine. Core 0 owns a ring in a region of RAM, core 1 consumes it;
# 10 000 frames go through, then core 0 drops the owner while core 1 is
# parked in a recv without a timeout. QEMU ends with the program's exit
# code (0: every frame arrived whole and in order, and the drop ended the
# park with the owner-gone error of §5.3).
#
#   tools/shm-region-two-cores.sh
#
# Needs qemu-system-aarch64 and the rustup target aarch64-unknown-none
# (both in ci/Dockerfile.rust); without them it fails, it does not skip.
set -uo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/crates/transport-shm/bare-metal/two-cores" || exit 2

command -v qemu-system-aarch64 >/dev/null || {
    echo "shm-region-two-cores: qemu-system-aarch64 is not installed" >&2
    exit 2
}
rustup target list --installed 2>/dev/null | grep -qx aarch64-unknown-none || {
    echo "shm-region-two-cores: the rustup target aarch64-unknown-none is not installed" >&2
    exit 2
}
# A target directory of its own: the workspace's may be set for the job.
# The program's Cargo.lock is committed; --locked builds what it names.
export CARGO_TARGET_DIR="${SHM_TWO_CORES_TARGET_DIR:-$PWD/target}"
cargo build --locked --release --color never 2>&1 | grep -E '^(error|warning)' -A6
[ "${PIPESTATUS[0]}" -eq 0 ] || exit 1

binary=$CARGO_TARGET_DIR/aarch64-unknown-none/release/zerodds-shm-region-two-cores
out=$(timeout 300 qemu-system-aarch64 -M virt -cpu cortex-a53 -smp 2 -m 128M \
    -nographic -nic none -semihosting -kernel "$binary" 2>&1)
status=$?
printf '%s\n' "$out"
if [ "$status" -ne 0 ]; then
    echo "shm-region-two-cores: QEMU ended with $status" >&2
    exit 1
fi
printf '%s\n' "$out" | grep -q "^zerodds-shm-region-two-cores: sent 10000, received 10000, .*, the owner's drop ended the parked recv" || {
    echo "shm-region-two-cores: the result line is missing" >&2
    exit 1
}

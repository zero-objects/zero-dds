#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# shm-ring-armv5te — zerodds-shm-transport-1.0 §6.4 on a core without
# 64-bit atomics: runs the tests of zerodds-transport-shm's ring and of the
# transport over a caller's region, built without `std` (the feature
# `alloc` only), for armv5te-unknown-linux-gnueabi under qemu-arm. That
# target has 32-bit atomics only, so the ring keeps its positions as two
# 32-bit words (`ring::split`), as a Cortex-M core does; the region tests
# run an owner and a consumer on two threads, the drop of an owner during
# a parked recv included. The test harness brings `std` for the threads;
# the library is the no_std build.
#
#   tools/shm-ring-armv5te.sh
#
# Needs the rustup target armv5te-unknown-linux-gnueabi, the cross linker
# arm-linux-gnueabi-gcc (gcc-arm-linux-gnueabi, libc6-dev-armel-cross) and
# qemu-arm (qemu-user), all in ci/Dockerfile.rust; without them it fails, it
# does not skip.
set -uo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)" || exit 2

target=armv5te-unknown-linux-gnueabi
for tool in qemu-arm arm-linux-gnueabi-gcc; do
    command -v "$tool" >/dev/null || {
        echo "shm-ring-armv5te: $tool is not installed" >&2
        exit 2
    }
done
rustup target list --installed 2>/dev/null | grep -qx "$target" || {
    echo "shm-ring-armv5te: the rustup target $target is not installed" >&2
    exit 2
}
# The measurement holds only on a target without 64-bit atomics.
cfg=$(rustc --print cfg --target "$target") || exit 1
if printf '%s\n' "$cfg" | grep -qx 'target_has_atomic="64"'; then
    echo "shm-ring-armv5te: $target has 64-bit atomics; the split positions would not run" >&2
    exit 1
fi
printf '%s\n' "$cfg" | grep -qx 'target_has_atomic="32"' || {
    echo "shm-ring-armv5te: $target has no 32-bit atomics" >&2
    exit 1
}

export CARGO_TARGET_ARMV5TE_UNKNOWN_LINUX_GNUEABI_LINKER=arm-linux-gnueabi-gcc
export CARGO_TARGET_ARMV5TE_UNKNOWN_LINUX_GNUEABI_RUNNER="qemu-arm -L /usr/arm-linux-gnueabi"
cargo nextest run --locked --profile merge --target "$target" \
    -p zerodds-transport-shm --no-default-features --features alloc --lib \
    --no-tests=fail --color never

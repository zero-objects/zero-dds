#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# no-std-claims-gate — every crate that claims `no-std` builds without
# `std`. A claim is the crates.io category or keyword in the manifest, or
# `#![no_std]` / `#![cfg_attr(..., no_std)]` in src/lib.rs.
#
#   tools/no-std-claims-gate.sh [--target thumbv7em-none-eabihf]
#
# The claim is read as a user reads it: `--no-default-features`, plus the
# crate's `alloc` feature where it has one, for a target that has no
# `std` at all. A host build is not the measurement: `#![no_std]` keeps
# `std` out of the crate, but a dependency built with its default
# features brings it back, and only a target without `std` says so.
#
# The CI job used to build three crates while 45 manifests claimed
# `no-std`; 21 of them did not build that way and lost the claim. This
# makes the set a function of the manifests, so a claim added later is
# measured from the first pipeline. The attribute counts as well: 73 crates
# carried it and 34 of them did not build that way, while only the manifest
# claims were measured.
set -uo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)" || exit 2

target=thumbv7em-none-eabihf
while [ $# -gt 0 ]; do
    case "$1" in
        --target) target="$2"; shift 2 ;;
        *) echo "no-std-claims-gate: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

failed=0
checked=0
# The category and the keyword are what crates.io shows, the attribute is
# what a reader of the source sees.
claims() {
    grep -lE '^(categories|keywords) = .*"no-std"' crates/*/Cargo.toml tools/*/Cargo.toml
    grep -lE '^#!\[(no_std|cfg_attr\(.*no_std\))\]' crates/*/src/lib.rs tools/*/src/lib.rs \
        | sed 's#/src/lib\.rs$#/Cargo.toml#'
}
for manifest in $(claims 2>/dev/null | sort -u); do
    name=$(grep -m1 '^name = ' "$manifest" | sed -E 's/^name = "(.*)"/\1/')
    features=""
    grep -qE '^alloc *=' "$manifest" && features="--features alloc"
    checked=$((checked + 1))
    # Uncoloured, so that the error lines below match in a CI log too.
    if out=$(cargo build -q --color never --target "$target" -p "$name" --no-default-features $features 2>&1); then
        echo "ok    $name"
    else
        failed=$((failed + 1))
        echo "FAIL  $name ($manifest claims no-std)"
        printf '%s\n' "$out" | grep -m3 -A2 '^error' | sed 's/^/      /'
    fi
done

echo
echo "no-std-claims-gate: $checked crates claim no-std, $failed do not build without std for $target"
[ "$checked" -gt 0 ] || { echo "no-std-claims-gate: no claim found — the manifest scan is broken" >&2; exit 3; }
[ "$failed" -eq 0 ]

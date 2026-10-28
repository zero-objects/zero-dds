#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run-tests.sh — the TypeScript runtime, type-checked and measured.
#
#   runtime/typescript/run-tests.sh
#
# Two halves. `tsc --noEmit` over `src/` is the first: this runtime is
# written for `strict` with `noUncheckedIndexedAccess` and
# `exactOptionalPropertyTypes`, and a type that only holds under looser
# settings is one a consumer cannot use.
#
# The second is `tests/against-recorded-octets.ts`, which holds the
# writer against `tests/interop-results/<case>/zerodds-rust/` — what
# the Rust column produced and what every other column in the ledger is
# measured against. A constant written into the test would be a third
# opinion about the wire.
#
# The third is `tests/against-recorded-values.ts`, which reads those same
# octets and holds what comes back against
# `tests/interop-corpus/<case>/values.json` — the case's own sample.
# Deliberately not against what the Rust probe reads back from its own
# bytes: two decoders that are wrong in the same way agree with each
# other, and a round trip through one column proves only that the column
# is self-consistent.
#
# The samples are written out by hand in both. They were written that
# way because the generator did not exist; they stay that way because
# they reach the runtime without it — a handful of cases held against
# the ledger with no generator, no `tsc` over generated source and no
# corpus walk in between, so a failure here is the runtime's.
#
# What this does **not** measure is the runtime the generated codec
# actually calls. `tools/wireprobe/wireprobe-typescript-corpus` is that:
# the whole corpus through the generator, `tsc` and node. Six defects in
# this runtime were found there and none of them was reachable from
# here, because a test that calls an operation itself calls it with the
# arguments it thinks are right — which is how `writeEnum`,
# `writeBitmask` and `writeBitset` each took theirs in the opposite
# order from the shared walk with this file green.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

# An installed `tsc`, and never `npx tsc`: without a local TypeScript,
# `npx` resolves the name from the npm registry and runs whatever answers
# to it — on a host with node and no compiler that is `tsc@2.0.4`, an
# unrelated package whose "This is not the tsc command you are looking
# for" then reads here as a compiler diagnostic. `ZERODDS_TSC` names one
# explicitly, the same variable the Rust gates in
# `crates/lang-rules/tests/typescript_*.rs` read.
if [ -n "${ZERODDS_TSC:-}" ]; then
    TSC="$ZERODDS_TSC"
elif [ -x "$here/node_modules/.bin/tsc" ]; then
    TSC="$here/node_modules/.bin/tsc"
else
    TSC="tsc"
fi
if ! command -v "$TSC" > /dev/null 2>&1; then
    echo "no TypeScript compiler: \`$TSC\` is not runnable (name one with ZERODDS_TSC)" >&2
    exit 1
fi

echo "-- tsc --noEmit over src/ (${TSC})"
"$TSC" --noEmit

echo "-- the writer against the recorded octets"
rm -rf dist-tests
# `--skipLibCheck` and the two globals declared in the test: this
# runtime depends on nothing from Node, because it has to run in a
# browser as well, and a `@types/node` in its dependencies would say
# otherwise.
"$TSC" --ignoreConfig --target ES2022 --module NodeNext \
    --moduleResolution NodeNext --strict --skipLibCheck \
    --outDir dist-tests tests/against-recorded-octets.ts \
    tests/against-recorded-values.ts tests/descriptor-runtime.ts \
    tests/wasm-bindings.ts tests/keyhash.ts src/*.ts 2>&1 \
    | grep -v 'TS2591' || true
node dist-tests/tests/against-recorded-octets.js

echo "-- the reader against the sample each case declares"
node dist-tests/tests/against-recorded-values.js

# The descriptor runtime — DDS-TS 1.0 chapter 8, measured against the
# nine marks Annex B §B.2 lists. Separate from the two above because it
# is about a different half of the runtime: those hold the octets
# against the ledger, this holds the reflection API against the clause.
echo "-- the descriptor runtime against Annex B §B.2"
node dist-tests/tests/descriptor-runtime.js

echo "-- the WASM bindings against Annex C §C.5"
node dist-tests/tests/wasm-bindings.js

echo "-- the KeyHash against RFC 1321 and DDSI-RTPS 2.5 §9.6.3.8"
node dist-tests/tests/keyhash.js

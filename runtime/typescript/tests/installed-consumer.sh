#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 3 ]]; then
    echo "usage: $0 RUNTIME_TARBALL IDLC_BINARY TSC_BINARY" >&2
    exit 2
fi
tarball=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
idlc=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
tsc=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
repo=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/zerodds-ts-installed.XXXXXX")
echo "Consumer artifacts: $work"
cp "$repo/runtime/typescript/tests/installed-consumer.ts" "$work/consumer.ts"
cp "$repo/runtime/typescript/tests/installed-package.json" "$work/package.json"
"$idlc" generate --lang typescript --encoding xcdr2 --out "$work" \
    "$repo/tests/interop-corpus/primitive-long/type.idl"
cd "$work"
npm install --offline --ignore-scripts --no-audit --no-fund "$tarball"
"$tsc" --strict --target ES2022 --module NodeNext --moduleResolution NodeNext \
    --outDir dist consumer.ts type.ts
node dist/consumer.js

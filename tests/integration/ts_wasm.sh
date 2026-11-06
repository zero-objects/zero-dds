#!/usr/bin/env bash
# The JavaScript tests of zerodds-ts-wasm (crates/ts-wasm/test): the codec
# through its wasm-bindgen binding in Node (smoke.test.mjs, against
# pkg-node/) and the DCPS-over-WebSocket facade of DDS-TS Annex C
# (dcps.test.ts, against src/ through tsx and an in-process `ws` server).
#
# Builds the crate for wasm32-unknown-unknown, generates pkg-node/ and
# pkg-web/ with the wasm-bindgen CLI (what `wasm-pack build --target nodejs`
# and `--target web` do, without wasm-pack fetching a CLI at run time) and
# runs `npm test`. The job's
# provision step (`wasm`, tests/integration/provision.sh) adds the target
# and the CLI where the image lacks them; the CLI's version has to be the
# wasm-bindgen crate's in Cargo.lock, or the generated glue does not match
# the module.
#
#     bash tests/integration/ts_wasm.sh
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"
target_dir="${CARGO_TARGET_DIR:-$repo/target}"

if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
    echo "ts_wasm: the Rust target wasm32-unknown-unknown is not installed (tests/integration/provision.sh wasm)" >&2
    exit 1
fi
locked=$(awk '/^name = "wasm-bindgen"$/ {getline; gsub(/"/, "", $3); print $3; exit}' Cargo.lock)
cli=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}') || true
if [ -z "$locked" ] || [ "$cli" != "$locked" ]; then
    echo "ts_wasm: wasm-bindgen CLI '${cli:-missing}', Cargo.lock has the crate at '$locked';" \
         "set WASM_BINDGEN_VERSION and its sha256 in tests/integration/provision.sh to it" >&2
    exit 1
fi

cargo build --locked -p zerodds-ts-wasm --target wasm32-unknown-unknown
wasm="$target_dir/wasm32-unknown-unknown/debug/zerodds_ts_wasm.wasm"
rm -rf crates/ts-wasm/pkg-node crates/ts-wasm/pkg-web
# pkg-node for the codec test, pkg-web for the facade (src/index.ts), as
# `npm run build:wasm` lays them out.
wasm-bindgen --target nodejs --out-dir crates/ts-wasm/pkg-node "$wasm"
wasm-bindgen --target web --out-dir crates/ts-wasm/pkg-web "$wasm"
# Beside the nodejs glue what wasm-pack writes there: that glue is CommonJS,
# and crates/ts-wasm/package.json declares "type": "module" for the
# directory above it. Without this Node reads the glue as an ES module with no exports
# (pipeline 8856, job 229164).
printf '{\n  "name": "zerodds-ts-wasm",\n  "type": "commonjs",\n  "main": "zerodds_ts_wasm.js"\n}\n' \
    > crates/ts-wasm/pkg-node/package.json

cd crates/ts-wasm
npm ci --no-audit --no-fund
npm test

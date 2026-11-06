#!/usr/bin/env bash
# Builds libzerodds where the two tests of zerodds-rpc that run a C++ program
# against it look for it (`$CARGO_TARGET_DIR/cpp-rpc`):
# tests/cpp_normative_headers.rs (DDS-RPC 1.0 §7.11.1, the normative headers)
# and tests/cpp_rpc_real_library.rs (the C++ binding over the real library).
# The integration job runs them in the step after this one; a test takes the
# library it finds there and builds only where there is none. With the build
# inside the test, both were cut off at 25 s in the merge lane.
#
#     bash tests/integration/rpc_cpp.sh
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}/cpp-rpc"
cd "$repo"
cargo build --locked -p zerodds-c-api --target-dir "$target"

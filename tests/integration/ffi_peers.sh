#!/usr/bin/env bash
# The subscribers of tests/integration/tests/bindings_ffi.rs
# (zerodds-ffi-loader-1.0 §5): libzerodds of zerodds-c-api, the C program
# against zerodds.h, the C++ program against crates/cpp/include/zerodds/dds.hpp,
# the C# program against crates/cs, and the packages of the koffi binding
# crates/ts-node (koffi, tsx) that the TypeScript program runs with. The
# Python program needs nothing built beyond libzerodds. Built once per job,
# before the tests, so that no test compiles inside its 25 s.
#
# Prints `ENV ZERODDS_LIB=<libzerodds.so>` and `ENV ZERODDS_FFI_PEERS=<dir>`
# for the nextest step after it (ci/run_integration.py).
#
#     bash tests/integration/ffi_peers.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
peers="$target/ffi-peers"
sources="$repo/tests/integration/peers/ffi"
mkdir -p "$peers"

echo "== cargo build -p zerodds-c-api --lib =="
(cd "$repo" && cargo build --locked -p zerodds-c-api --lib)
lib="$target/debug"

echo "== the C and the C++ subscriber =="
"${CC:-cc}" -std=c99 -D_POSIX_C_SOURCE=200809L -Wall -Wextra \
  -I "$repo/crates/zerodds-c-api/include" "$sources/sub.c" \
  -L "$lib" -Wl,-rpath,"$lib" -lzerodds -o "$peers/sub_c"
# zerodds/dds.hpp includes the C++ wire runtime's header.
"${CXX:-c++}" -std=c++17 -Wall -Wextra \
  -I "$repo/crates/zerodds-c-api/include" -I "$repo/crates/cpp/include" \
  -I "$repo/runtime/cpp/include" "$sources/sub.cpp" \
  "$repo/runtime/cpp/src/writer.cpp" "$repo/runtime/cpp/src/reader.cpp" \
  "$repo/runtime/cpp/src/md5.cpp" "$repo/runtime/cpp/src/float128.cpp" \
  -L "$lib" -Wl,-rpath,"$lib" -lzerodds -lpthread -ldl -lm -o "$peers/sub_cpp"

echo "== the C# subscriber =="
dotnet build "$sources/csharp/FfiSub.csproj" -c Release --nologo -v quiet \
  -o "$peers/csharp"

echo "== the packages of crates/ts-node =="
# The runner's registry mirror when it resolves, for this command only (no
# `.npmrc` left in the tree).
if [ -n "${NEXUS_NPM_REGISTRY:-}" ]; then
  nexus_host=$(printf %s "$NEXUS_NPM_REGISTRY" | sed -E 's#^https?://([^/:]+).*#\1#')
  if getent hosts "$nexus_host" >/dev/null 2>&1; then
    export npm_config_registry="$NEXUS_NPM_REGISTRY"
  fi
fi
(cd "$repo/crates/ts-node" && npm ci --no-audit --no-fund)

echo "ENV ZERODDS_LIB=$lib/libzerodds.so"
echo "ENV ZERODDS_FFI_PEERS=$peers"

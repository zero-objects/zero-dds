#!/usr/bin/env bash
# The C and C++ SDK as a user builds it: CMake configures the repository as a
# top-level project, builds libzerodds (shared) and the C++ PSM, CTest runs
# the tests the CMake files register, `cmake --install` stages the SDK, and
# cmake/tests/sdk-consumer builds a C and a C++ program against the staged
# package alone (find_package(zerodds CONFIG REQUIRED)) and runs them. The
# ctypes loader of the Python package is tested in the job integration-python.
#
# The oracle is the toolchain's own: CMake's package contract, the C and C++
# compilers and the dynamic linker resolve every header, target and symbol
# the SDK documents (docs/specs/zerodds-c-api-1.0.md, the zeroddsConfig
# targets zerodds::zerodds-c and zerodds::zerodds-cpp); CTest runs the
# programs the CMake files register.
#
#     bash tests/integration/sdk_cmake.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The build tree persists per runner slot where the runner offers a cache
# directory: CMake runs cargo with the build tree as its target directory, and
# a cold libzerodds build is most of the job.
if [ -d /zerodds-cache ]; then
  build="/zerodds-cache/cmake-sdk-${CI_CONCURRENT_PROJECT_ID:-0}"
else
  build="${TMPDIR:-/tmp}/zerodds-sdk-build"
fi
work="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-sdk.XXXXXX")"
trap 'rm -rf "$work"' EXIT
stage="$work/stage"

# A cache from another checkout path is refused by CMake; start that one over.
if [ -f "$build/CMakeCache.txt" ] && ! grep -qx "CMAKE_HOME_DIRECTORY:INTERNAL=$repo" "$build/CMakeCache.txt"; then
  rm -rf "$build"
fi

# CMake runs cargo from the build tree, where rustup does not see the
# repository's rust-toolchain.toml; name the pinned toolchain explicitly.
if command -v rustup >/dev/null 2>&1; then
  RUSTUP_TOOLCHAIN="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo/rust-toolchain.toml")"
  export RUSTUP_TOOLCHAIN
fi

echo "== configure (Debug, shared libzerodds, tests on) =="
# The cargo of the toolchain this job builds with (rust-toolchain.toml), not
# whichever one CMake's search finds first.
cmake -S "$repo" -B "$build" \
  -DCARGO_EXECUTABLE="$(command -v cargo)" \
  -DCMAKE_BUILD_TYPE=Debug \
  -DBUILD_SHARED_LIBS=ON \
  -DZERODDS_BUILD_CPP=ON \
  -DZERODDS_BUILD_IDLC=ON \
  -DZERODDS_BUILD_EXAMPLES=OFF \
  -DZERODDS_BUILD_TESTS=ON \
  -DBUILD_TESTING=ON

echo "== build =="
cmake --build "$build" --parallel "${CARGO_BUILD_JOBS:-8}"

echo "== ctest =="
python3 "$repo/tests/integration/with_domains.py" 4 -- \
  ctest --test-dir "$build" --output-on-failure --timeout 25 --no-tests=error

echo "== install and consume =="
cmake --install "$build" --prefix "$stage"
cmake -S "$repo/cmake/tests/sdk-consumer" -B "$work/consumer" -DCMAKE_PREFIX_PATH="$stage"
cmake --build "$work/consumer"
LD_LIBRARY_PATH="$stage/lib:${LD_LIBRARY_PATH:-}" "$work/consumer/use_c"
LD_LIBRARY_PATH="$stage/lib:${LD_LIBRARY_PATH:-}" "$work/consumer/use_cpp"

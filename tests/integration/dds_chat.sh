#!/usr/bin/env bash
# The codecs of the dds-chat tutorial (examples/tutorials/dds-chat) against
# its fixtures, and the fixtures against the clauses they come from.
#
# shared/fixtures/fixtures.py derives ten serialized payloads of the three
# `@appendable` types of shared/chat.idl from the serialization rules of
# DDS-XTypes 1.3 (XCDR2 little endian, D_CDR2_LE); `--check` re-derives
# them and compares with the committed files. Every port then reads every
# file to its values and writes the values back to the same octets: C++
# (three directories, header-only, g++), Python (two), TypeScript (ts-node
# and, through the DDS-TS-1.0 WASM codec, ts-browser and web-spa), Rust
# (the `no_std` codec of apps/embedded-mcu and the reference codec of
# code/), Java (two, Maven) and C# (two, dotnet), each port's fixture test
# alone, without its live tests; Flutter in integration-flutter; Cyclone
# DDS's serialization of the same samples in vendor-live-dcps
# (`fixtures.py --cyclone`, in the image that carries the binding).
#
#     bash tests/integration/dds_chat.sh [results-dir]
set -uo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"
results="${1:-integration-results/integration-dds-chat}"
mkdir -p "$results" && results="$(cd "$results" && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo/target}"
chat="$repo/examples/tutorials/dds-chat"
failed=()

step() {
    local name="$1"; shift
    local started=$SECONDS
    echo "=== $name"
    if "$@"; then
        echo "=== ok: $name ($((SECONDS - started)) s)"
    else
        echo "=== FAILED: $name ($((SECONDS - started)) s)"
        failed+=("$name")
    fi
}

# The C# test projects target net8.0 and net9.0; the job installs the 9.0
# SDK, whose runtime runs both.
export DOTNET_ROLL_FORWARD=Major

step "fixtures from the clauses" python3 "$chat/shared/fixtures/fixtures.py" --check

cpp() {
    local dir="$1" out="$results/cpp-$(basename "$1")"
    g++ -std=c++17 -Wall -Wextra -Wpedantic -Werror \
        "-DFIXTURES_DIR=\"$chat/shared/fixtures\"" \
        "$chat/$dir/test/fixtures_test.cpp" -o "$out" && "$out"
}
for dir in ports/cpp-tui ports/cpp-qt6 apps/qt-desktop; do
    step "c++ $dir" cpp "$dir"
done

python_port() {
    (cd "$chat/$1" && python3 -m pytest tests/test_fixtures.py -q -p no:cacheprovider \
        --junitxml "$results/junit-pytest-$(basename "$1").xml")
}
for dir in ports/python-cli ports/python-gui; do
    step "python $dir" python_port "$dir"
done

# tsx runs the TypeScript sources as they are; one copy for the three
# ports, outside the tree.
tsx_dir="$(mktemp -d)"
step "npm tsx" bash -c "cd '$tsx_dir' && npm init -y >/dev/null && npm install --no-audit --no-fund tsx@4.19.2 >/dev/null"
ts_port() {
    (cd "$tsx_dir" && node --import tsx --test "$chat/$1")
}
step "ts-node" ts_port ports/ts-node/test/fixtures.test.ts

# ts-browser and web-spa write through the DDS-TS-1.0 WASM codec, the
# Node build of zerodds-ts-wasm (as tests/integration/ts_wasm.sh builds it).
wasm_pkg() {
    cargo build --locked -p zerodds-ts-wasm --target wasm32-unknown-unknown || return 1
    local pkg="$tsx_dir/dds-ts-wasm"
    rm -rf "$pkg"
    wasm-bindgen --target nodejs --out-dir "$pkg" \
        "$target_dir/wasm32-unknown-unknown/debug/zerodds_ts_wasm.wasm" || return 1
    printf '{\n  "name": "dds-ts-wasm",\n  "type": "commonjs",\n  "main": "zerodds_ts_wasm.js"\n}\n' \
        > "$pkg/package.json"
    for port in ports/ts-browser apps/web-spa; do
        mkdir -p "$chat/$port/node_modules"
        rm -rf "$chat/$port/node_modules/dds-ts-wasm"
        ln -s "$pkg" "$chat/$port/node_modules/dds-ts-wasm"
    done
}
step "dds-ts-wasm for Node" wasm_pkg
step "ts-browser" ts_port ports/ts-browser/test/cross-port-fixtures.test.ts
step "web-spa" ts_port apps/web-spa/test/fixtures.test.ts
rm -rf "$chat/ports/ts-browser/node_modules" "$chat/apps/web-spa/node_modules"

# The Rust ports are workspaces of their own, each with its lock file; they
# share the job's target directory.
step "rust apps/embedded-mcu" cargo test \
    --manifest-path "$chat/apps/embedded-mcu/Cargo.toml" --test codec
step "rust code" cargo test --locked --manifest-path "$chat/code/Cargo.toml" \
    --test fixture_byte_identity --test ch11_evolution --test ch12_interop
step "rust 00-base/03-rust-tui" cargo check --locked \
    --manifest-path "$chat/00-base/03-rust-tui/Cargo.toml"
# The bridges carry a fixture as opaque octets (no lock file of their own).
for bridge in amqp coap grpc mqtt websocket; do
    step "rust integrations/$bridge" cargo test \
        --manifest-path "$chat/integrations/$bridge/Cargo.toml" --test codec
done

java_port() {
    local pom="$chat/$1/pom.xml"
    mvn -B --no-transfer-progress -f "$pom" -Dtest=FixturesTest \
        -Dsurefire.failIfNoSpecifiedTests=false clean test || return 1
    for report in "$chat/$1"/target/surefire-reports/TEST-*.xml; do
        cp "$report" "$results/junit-maven-$(basename "$1")-$(basename "$report")"
    done
}
for dir in ports/java-cli ports/java-backend; do
    step "java $dir" java_port "$dir"
done

csharp_port() {
    dotnet test "$chat/$1" --filter "FullyQualifiedName~FixturesTests" \
        --logger "trx;LogFileName=$(basename "$(dirname "$1")").trx" --results-directory "$results/dotnet"
}
step "c# ports/csharp-cli" csharp_port ports/csharp-cli/test/DdsChatCsharpCliTests.csproj
step "c# ports/csharp-wpf" csharp_port ports/csharp-wpf/test/DdsChatWpfTests.csproj

rm -rf "$tsx_dir"
if [ "${#failed[@]}" -gt 0 ]; then
    printf 'failed: %s\n' "${failed[@]}"
    exit 1
fi

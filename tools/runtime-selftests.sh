#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# runtime-selftests.sh — every language runtime's own self-test, one
# line per runtime with its exit status.
#
#   tools/runtime-selftests.sh            all twenty
#   tools/runtime-selftests.sh go zig     those
#
# The runtimes under `runtime/<lang>` are what every generated codec
# calls, and each carries a self-test against recorded octets. No CI job
# runs them (`internal/release/OPEN-ITEMS.md`); this is the one command
# that does, with the toolchains the caller's environment provides. A
# runtime whose toolchain is missing is reported as such, not as passed.
#
# Exit status: 0 when every runtime asked for passed, 1 otherwise.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

ALL="c python cpp csharp java typescript kotlin node go swift zig nim d julia ada ada-83 ocaml fsharp elixir lua"

# needs <tool>... — every tool on PATH, or the runtime is not run.
needs() {
    local tool
    for tool in "$@"; do
        command -v "$tool" > /dev/null 2>&1 || { echo "missing $tool"; return 1; }
    done
}

run_one() {
    local name=$1
    cd "$ROOT" || return 1
    case "$name" in
        c)          needs make cc && make -C runtime/c check ;;
        python)     needs python3 && (cd runtime/python && python3 -m unittest discover -s tests) ;;
        cpp)        needs make g++ && make -C runtime/cpp check ;;
        csharp)     needs dotnet && bash runtime/csharp/run-tests.sh ;;
        java)       needs javac java && bash runtime/java/run-tests.sh ;;
        typescript) needs node tsc && bash runtime/typescript/run-tests.sh ;;
        kotlin)     needs kotlinc java \
                        && kotlinc -Werror runtime/kotlin/ZdWire.kt runtime/kotlin/SelfTest.kt \
                               -include-runtime -d "$work/kotlin.jar" \
                        && java -cp "$work/kotlin.jar" zerodds.wire.SelfTestKt ;;
        node)       needs node && node runtime/node/selftest.js ;;
        go)         needs go && (cd runtime/go && go test ./...) ;;
        swift)      needs swift && (cd runtime/swift && swift test) ;;
        zig)        needs zig && (cd runtime/zig && zig test zdwire.zig) ;;
        nim)        needs nim && nim c -r --hints:off --nimcache:"$work/nimcache" \
                        -o:"$work/nim-selftest" runtime/nim/zdwire.nim ;;
        d)          if command -v ldc2 > /dev/null 2>&1; then
                        (cd "$work" && ldc2 -unittest -main -run "$ROOT/runtime/d/zdwire.d")
                    elif command -v dmd > /dev/null 2>&1; then
                        (cd "$work" && dmd -unittest -main -run "$ROOT/runtime/d/zdwire.d")
                    else
                        # Debian's D compiler, the one the CI image installs.
                        needs gdc && gdc -funittest -fmain -o "$work/d-selftest" \
                            runtime/d/zdwire.d && "$work/d-selftest"
                    fi ;;
        julia)      needs julia && julia runtime/julia/test/runtests.jl ;;
        ada)        needs gprbuild && (cd runtime/ada && gprbuild -q -P zd_wire.gpr \
                        && obj/zd_wire_selftest) ;;
        ada-83)     needs gprbuild && (cd runtime/ada-83 && gprbuild -q -P zd_wire83.gpr \
                        && obj/zd_wire83_selftest) ;;
        ocaml)      if command -v dune > /dev/null 2>&1; then
                        (cd runtime/ocaml && dune test --force)
                    else
                        # The same two files `tests/dune` builds, without dune.
                        needs ocamlfind && cp runtime/ocaml/zdwire.ml \
                            runtime/ocaml/tests/zdwire_selftest.ml "$work/" \
                            && (cd "$work" && ocamlfind ocamlopt -package unix -linkpkg \
                                    zdwire.ml zdwire_selftest.ml -o ocaml-selftest \
                                && ./ocaml-selftest)
                    fi ;;
        fsharp)     needs dotnet && (cd runtime/fsharp && dotnet fsi --exec tests/ZdWireSelfTest.fsx) ;;
        elixir)     needs mix && (cd runtime/elixir && mix test) ;;
        # Debian ships no unversioned `lua`; `LUA=lua5.4` names its binary.
        lua)        needs "${LUA:-lua}" && (cd runtime/lua && "${LUA:-lua}" tests/selftest.lua) ;;
        *)          echo "runtime-selftests: \`$name\` is not a runtime here" >&2; return 2 ;;
    esac
}

asked="${*:-$ALL}"
failed=0
for name in $asked; do
    log="$work/$name.log"
    if run_one "$name" > "$log" 2>&1; then
        printf '%-11s ok\n' "$name"
    else
        status=$?
        failed=1
        printf '%-11s FAILED (exit %s)\n' "$name" "$status"
        tail -n 15 "$log" | sed 's/^/    /'
    fi
done
exit "$failed"

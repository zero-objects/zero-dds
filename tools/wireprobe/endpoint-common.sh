# shellcheck shell=bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# endpoint-common.sh — the body of the seventeen hand-written endpoint
# probes.
#
# Sourced, not executed. `endpoint_probe <lang> "$@"` is the whole of
# `wireprobe-endpoint-<lang>`; the seventeen wrappers exist so a case runs
# through any column by substituting the program name, which is the
# contract `docs/specs/zerodds-interop-suite-1.0.md` §3a states and the
# ledger machinery relies on.
#
# One file and not seventeen. Three strands built this probe against the
# same contract and each brought its own driver; the fold kept this one
# because the caching, the exit codes and the run discipline are the
# same question seventeen times and seventeen copies of an answer are
# seventeen places for it to drift. What is genuinely per language — which
# compiler runs, which SDK source is linked, whether the toolchain
# produces a native executable or something a launcher has to start — is
# `endpoint_build` and `endpoint_sdk_sources`, and nothing else is.
#
# ## What it does
#
# `wireprobe-endpoint-gen` writes one program in the binding's own
# language — nothing but calls into `endpoints/<lang>` — and this
# compiles it against that binding and runs it. The octets come out of
# the binding. Nothing in this script and nothing in the generated
# program decides a wire question.
#
# ## The cache is keyed on the generated source
#
# A corpus sweep is 376 cases in ten configurations per language, and a
# `gnatmake` is a second and a half. Five of the ten configurations
# collapse onto the same program — the encoding names differ, the
# framing a `@final` struct of narrow scalars gets does not — so keying
# the cache on the text of the generated source rather than on the case
# name turns ~3760 compiles per language into a few hundred. The SDK's
# own sources go into the key too, so an edit under `endpoints/`
# invalidates every entry rather than measuring the previous build.
#
# ## The exit codes
#
# `docs/specs/zerodds-verdicts-1.0.md` §"The exit codes a probe answers
# in": 3 is `no-form` — the binding's write surface has no shape for
# this case, which is the extent of the binding and not a failure of it
# — 6 is `harness-gap`, 7 is `generator-defect`, and the reason is on
# standard error either way. Nothing here fails silently.
#
# 125 is infrastructure failure (timeout, signal/OOM, spawn/resource failure),
# never a binding refusal or proof of a detected mutation.
# 9 is `toolchain-absent` and is **not a measurement**: this host has not
# got the binding's compiler, which says nothing about the case. It used
# to be 7 — `endpoint_build`'s `command -v` guards answered 1 and the
# caller read every non-zero build status as "the generated program did
# not build" — so a host without `zig` recorded `generator-defect` for
# all 3 760 of that column's cells, a verdict which asserts the compiler
# ran. Every sweep that calls these probes stops on 9 and records no
# cell; see "Why 9 exists" in the spec.

endpoint_usage() {
    cat <<'USAGE'
wireprobe-endpoint-<lang> — one IDL type and one JSON sample, through one
hand-written endpoint SDK under endpoints/.

    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --values <file>         the sample, as JSON
    --encoding <name>       plain-cdr1 | pl-cdr1 | plain-cdr2 |
                            delimited-cdr | pl-cdr2
    --endian le|be
    --numbers bits|natural  which spelling of an integer argument the
                            generated program hands the binding; see
                            `wireprobe-endpoint-gen --help`
    --out <file>            where the octets go; standard output as hex
                            when it is left off

Exits 3 when the binding's write surface has no form for the case, 6 when
the program would not run, 7 when the generated program does not build,
and 9 when this host has not got the binding's compiler at all — which is
not a measurement, and which every sweep stops on rather than records.
USAGE
}

# endpoint_probe <lang> <argv...>
endpoint_probe() (
    local lang="$1"; shift
    local here root gen out="" idl="" type_name="" values="" encoding="plain-cdr2" endian="le"
    local numbers="${ENDPOINT_NUMBERS:-bits}"

    here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    root="$(cd "$here/../.." && pwd)"
    source "$here/runtime-cache.sh"

    while [ $# -gt 0 ]; do
        case "$1" in
            --idl) idl="$2"; shift 2 ;;
            --type) type_name="$2"; shift 2 ;;
            --values) values="$2"; shift 2 ;;
            --encoding) encoding="$2"; shift 2 ;;
            --endian) endian="$2"; shift 2 ;;
            --numbers) numbers="$2"; shift 2 ;;
            --out) out="$2"; shift 2 ;;
            --help|-h) endpoint_usage; return 0 ;;
            *) echo "wireprobe-endpoint-$lang: \`$1\` is not an option" >&2; return 2 ;;
        esac
    done
    if [ -z "$idl" ] || [ -z "$type_name" ] || [ -z "$values" ]; then
        echo "wireprobe-endpoint-$lang: --idl, --type and --values are required" >&2
        return 2
    fi

    # One process-group owner for the complete cell, including preparation.
    # Leaf phases have shorter grace periods so nested cleanup finishes first.
    if [ "${ENDPOINT_CELL_SUPERVISED:-0}" != 1 ]; then
        local args=(--idl "$idl" --type "$type_name" --values "$values"
                    --encoding "$encoding" --endian "$endian" --numbers "$numbers")
        [ -z "$out" ] || args+=(--out "$out")
        ENDPOINT_CELL_SUPERVISED=1 python3 "$here/probe-process.py" \
            --timeout "${ENDPOINT_CELL_TIMEOUT:-300}" --grace 3 -- \
            bash -c 'source "$1"; shift; endpoint_probe "$@"' _ \
            "$here/endpoint-common.sh" "$lang" "${args[@]}"
        return $?
    fi
    # Budget VM schedulers, not only the outer worker count. Erlang otherwise
    # starts one scheduler per visible host core for every concurrent probe.
    export ERL_FLAGS="+S 2:2 +SDcpu 1 +SDio 1 +A 1"
    export CARGO_BUILD_JOBS=1 GOMAXPROCS=1 JULIA_NUM_THREADS=1

    gen="${WIREPROBE_ENDPOINT_GEN:-${CARGO_TARGET_DIR:-$root/target}/debug/wireprobe-endpoint-gen}"
    if [ -z "${WIREPROBE_ENDPOINT_GEN:-}" ]; then
        # Built when it is missing *or stale*: a generator older than the
        # sources it came from measures the previous build and says
        # nothing about it.
        # shellcheck source=generator.sh
        . "$here/generator.sh"
        probe_generator zerodds-wireprobe-endpoint-gen "$gen" "$root" || return 125
    fi

    local work stage="" code=0
    work="$(mktemp -d)" || return 125
    trap '[ -z "$stage" ] || rm -rf "${stage:?}"; rm -rf "${work:?}"' EXIT
    trap 'exit 125' HUP INT TERM

    python3 "$here/probe-process.py" --timeout "${ENDPOINT_BUILD_TIMEOUT:-180}" --grace 1 -- \
        "$gen" --lang "$lang" --idl "$idl" --type "$type_name" --values "$values" \
        --encoding "$encoding" --endian "$endian" --numbers "$numbers" \
        --out-dir "$work" || code=$?
    if [ "$code" != 0 ]; then
        return "$code"
    fi

    local source key cache
    source="$(endpoint_source_name "$lang")"
    [ -f "$work/$source" ] || { echo "endpoint generator produced no source" >&2; return 7; }
    # The generated text plus every source of the SDK it calls: a binding
    # that changed has to invalidate the binaries built against it.
    key=$( { cat "$work/$source"; endpoint_sdk_sources "$root" "$lang" | sort | xargs cat; } \
        | shasum | cut -c1-32 )
    cache="${ZERODDS_ENDPOINT_CACHE:-${TMPDIR:-/tmp}/zd-endpoint}/$lang/$key"

    local companions=(probe)
    case "$lang" in
        kotlin) companions+=(classes/ProbeKt.class classes/zerodds/Writer.class classes/zerodds/Reader.class
                            classes/zerodds/Encoding.class classes/zerodds/Endian.class) ;;
        java) companions+=(classes/Probe.class classes/Zdw.class 'classes/Zdw$Writer.class'
                          'classes/Zdw$Reader.class' 'classes/Zdw$Pid.class') ;;
        node) companions+=(probe.js zerodds.js) ;;
        fsharp) companions+=(probe.fsx zerodds.fs) ;;
        julia) companions+=(probe.jl zerodds.jl) ;;
        lua) companions+=(probe.lua zerodds.lua) ;;
        elixir) companions+=(probe.exs beam/Elixir.ZeroDDS.Wire.beam beam/Elixir.ZeroDDS.Endpoint.beam
                            beam/Elixir.ZeroDDS.Client.beam beam/Elixir.ZeroDDS.AsyncReader.beam
                            beam/Elixir.ZeroDDS.MemTransport.beam) ;;
        python) companions+=(probe.py zerodds_wire.py) ;;
    esac
    if ! runtime_cache_ready "$cache" -p "${companions[@]}"; then
        mkdir -p "$(dirname "$cache")" || return 125
        stage=$(mktemp -d "$cache.tmp.XXXXXX") || return 125
        cp "$work/$source" "$stage/" || return 125
        # The fourth argument is where the entry will *be* once it is
        # renamed into place. Four of the ten bindings compile to
        # something that is not a native executable — class files, a
        # script, an fsx — so their `probe` is a launcher that has to
        # name the entry by absolute path, and the path it is built in
        # is not the path it will be run from.
        local build_code=0
        python3 "$here/probe-process.py" --timeout "${ENDPOINT_BUILD_TIMEOUT:-180}" --grace 1 -- \
            bash -ec 'source "$1"; shift; endpoint_build "$@"' _ \
            "$here/endpoint-common.sh" "$lang" "$root" "${stage:?}" "$cache" \
            > "$work/build.log" 2>&1 || build_code=$?
        if [ "$build_code" = 125 ] || endpoint_resource_failure "$work/build.log"; then
            echo "wireprobe-endpoint-$lang: infrastructure failure during build" >&2
            head -10 "$work/build.log" >&2
            return 125
        fi
        # 9 before 7, and the two are not the same finding. A host
        # without `zig` used to reach the arm below and every one of the
        # zig column's cells was recorded as `generator-defect` — "its
        # front end accepted the case and its own generated code does not
        # build", which says the program was compiled and it never was.
        # `endpoint_build`'s `command -v` guards answer 9 for exactly
        # this, and 9 is not a measurement:
        # `docs/specs/zerodds-verdicts-1.0.md`, "Why 9 exists".
        if [ "$build_code" = 9 ]; then
            echo "wireprobe-endpoint-$lang: this host has not got the toolchain: \
$(head -1 "$work/build.log")" >&2
            rm -rf "${stage:?}"
            return 9
        fi
        if [ "$build_code" != 0 ]; then
            echo "wireprobe-endpoint-$lang: the generated program did not build" >&2
            grep -m10 -E "error|Error|warning|Warning" "$work/build.log" >&2 \
                || head -5 "$work/build.log" >&2
            rm -rf "${stage:?}"
            return 7
        fi
        if ! runtime_cache_publish "$stage" "$cache" -p "${companions[@]}"; then
            rm -rf "${stage:?}"
            return 6
        fi
        rm -rf "${stage:?}"
    fi

    # The program writes `out.bin` beside itself, so it is run in a
    # directory of its own: the byte order is compiled in — Ada 83 has no
    # `Ada.Command_Line` — and a shared working directory would let two
    # concurrent probes overwrite each other's octets.
    local run="$work/run"
    mkdir -p "$run" || return 125
    cp "$cache/probe" "$run/probe" || return 125
    local run_code=0
    ( cd "$run" && python3 "$here/probe-process.py" \
        --timeout "${ENDPOINT_RUN_TIMEOUT:-30}" --grace 1 -- ./probe ) \
        > "$work/run.out" 2> "$work/run.err" || run_code=$?
    if [ "$run_code" = 125 ] || endpoint_resource_failure "$work/run.err" "$work/run.out"; then
        echo "wireprobe-endpoint-$lang: infrastructure failure during execution" >&2
        head -10 "$work/run.err" >&2
        return 125
    fi
    if [ "$run_code" != 0 ]; then
        echo "wireprobe-endpoint-$lang: the program did not run" >&2
        head -5 "$work/run.err" >&2
        head -5 "$work/run.out" >&2
        return 6
    fi
    # Standard error is read and not discarded. Four of these fifteen
    # toolchains report a diagnostic at run time and carry on — Elixir
    # prints a warning and returns 0, Julia's non-deprecation reports
    # arrive here rather than at compile time — and a column that took
    # the octets anyway would be reporting a program whose diagnostics
    # nobody read. `generator-defect`, because the program is this
    # generator's.
    if [ -s "$work/run.err" ]; then
        echo "wireprobe-endpoint-$lang: the program wrote to standard error" >&2
        head -10 "$work/run.err" >&2
        return 7
    fi
    if [ ! -f "$run/out.bin" ]; then
        echo "wireprobe-endpoint-$lang: the program wrote no octets" >&2
        return 6
    fi

    if [ -n "$out" ]; then
        cp "$run/out.bin" "$out"
    else
        od -An -tx1 -v "$run/out.bin" | tr -d ' \n'; echo
    fi
    return 0
)

# Compilers sometimes turn a killed subprocess into ordinary exit 1. Preserve
# the infrastructure meaning of explicit resource diagnostics in that case.
endpoint_resource_failure() {
    grep -qiE 'out of memory|cannot allocate memory|cannot allocate [0-9]+ bytes|failed to create.*thread|resource temporarily unavailable|no space left on device|(^|[[:space:]])killed([[:space:]]|$)|unable to create.*thread' "$@"
}

endpoint_source_name() {
    case "$1" in
        zig) echo probe.zig ;;
        nim) echo probe.nim ;;
        d) echo probe.d ;;
        ada|ada-83|ada-native) echo probe.adb ;;
        kotlin) echo Probe.kt ;;
        node) echo probe.js ;;
        fsharp) echo probe.fsx ;;
        swift) echo probe.swift ;;
        go) echo probe.go ;;
        julia) echo probe.jl ;;
        lua) echo probe.lua ;;
        ocaml) echo probe.ml ;;
        elixir) echo probe.exs ;;
        python) echo probe.py ;;
        java) echo Probe.java ;;
    esac
}

# The tree the seventeen SDKs are read from — the directory that holds
# `endpoints/`, not `endpoints/` itself.
#
# The repository root unless `ZERODDS_ENDPOINT_ROOT` names another, which
# is how `endpoint-mutation-gate` points a whole sweep at a copy with one
# defect planted in it. One variable and not fifteen: the gate mutates
# every binding at the same clause, and fifteen variables would be
# fifteen chances to leave one pointing at the original and read the
# green that follows as a caught mutation.
#
# It reaches `endpoint_sdk_sources` as well as `endpoint_build`, so the
# cache key is the mutated source's and a planted defect can never be
# served a binary built against the original.
endpoint_sdk_root() {
    printf '%s\n' "${ZERODDS_ENDPOINT_ROOT:-$1}"
}

# The SDK sources a binding's octets come out of, for the cache key.
endpoint_sdk_sources() {
    local root="$1" lang="$2"
    case "$lang" in
        zig) find "$(endpoint_sdk_root "$root")/endpoints/zig/src" -name '*.zig' ;;
        nim) echo "$(endpoint_sdk_root "$root")/endpoints/nim/zerodds.nim" ;;
        d) echo "$(endpoint_sdk_root "$root")/endpoints/d/zerodds.d" ;;
        # `zdw_xcdr.*` as well as `zdw.ads`: the framing half of Stage 1
        # lives there, so a key that named only the C bindings would
        # serve a cached probe built against a different member header
        # and call its octets current.
        ada) find "$(endpoint_sdk_root "$root")/endpoints/ada/src" \
                  -name 'zdw.ads' -o -name 'zdw_xcdr.ads' -o -name 'zdw_xcdr.adb'
             find "$(endpoint_sdk_root "$root")/endpoints/c/src" -name 'zerodds_wire.c'
             find "$(endpoint_sdk_root "$root")/endpoints/c/include" -name 'zerodds_wire.h' ;;
        ada-83) find "$(endpoint_sdk_root "$root")/endpoints/ada-83/src" -name 'zerodds_ada83_wire.*' ;;
        ada-native) find "$(endpoint_sdk_root "$root")/endpoints/ada-native/src" -name 'zerodds_native_wire.*' ;;
        kotlin) echo "$(endpoint_sdk_root "$root")/endpoints/kotlin/src/Zerodds.kt" ;;
        node) echo "$(endpoint_sdk_root "$root")/endpoints/node/zerodds.js" ;;
        fsharp) echo "$(endpoint_sdk_root "$root")/endpoints/fsharp/zerodds.fs" ;;
        swift) echo "$(endpoint_sdk_root "$root")/endpoints/swift/Sources/Zerodds/Zerodds.swift" ;;
        # Every non-test source of the package, and not `wire.go` alone:
        # `go build` compiles the whole `zeroddsendpoint` package, so a
        # key that named one file would serve a binary built against a
        # different `sync.go` and call the octets current.
        go) find "$(endpoint_sdk_root "$root")/endpoints/go" -maxdepth 1 \
                -name '*.go' -not -name '*_test.go' ;;
        julia) echo "$(endpoint_sdk_root "$root")/endpoints/julia/zerodds.jl" ;;
        lua) echo "$(endpoint_sdk_root "$root")/endpoints/lua/zerodds.lua" ;;
        ocaml) echo "$(endpoint_sdk_root "$root")/endpoints/ocaml/zerodds.ml" ;;
        elixir) echo "$(endpoint_sdk_root "$root")/endpoints/elixir/lib/zerodds.ex" ;;
        python) echo "$(endpoint_sdk_root "$root")/endpoints/python/zerodds_wire.py" ;;
        java) echo "$(endpoint_sdk_root "$root")/endpoints/java/Zdw.java" ;;
    esac
}

# endpoint_build <lang> <root> <dir> [final] — compile <dir>/probe.<ext>
# to <dir>/probe. `final` is where <dir> will be renamed to, which only
# the four launcher-based bindings need.
#
# Warnings are errors wherever the front end can say so. `-gnatwae` is
# `-gnatwa` plus `-gnatwe`; gdc takes `-Werror`; Zig has no lenient mode
# to leave. Nim has no flag that covers every warning at once, so its
# output is read instead and a `Warning:` fails the build — the same rule
# by a different route.
endpoint_build() {
    local lang="$1" root="$2" dir="$3" final="${4:-$3}"
    case "$lang" in
        zig)
            local zig="${ZIG:-zig}"
            command -v "$zig" >/dev/null || { echo "no \`$zig\` in PATH"; return 9; }
            # zerodds.zig imports reliable.zig, so the whole module
            # directory goes next to the probe and not one file of it.
            cp "$(endpoint_sdk_root "$root")"/endpoints/zig/src/*.zig "$dir/"
            ( cd "$dir" && "$zig" build-exe \
                --cache-dir "$dir/zig-cache" \
                --global-cache-dir "${ZIG_GLOBAL_CACHE_DIR:-${TMPDIR:-/tmp}/zd-zig-cache}" \
                -femit-bin=probe probe.zig )
            ;;
        nim)
            local nim="${NIM:-nim}"
            command -v "$nim" >/dev/null || { echo "no \`$nim\` in PATH"; return 9; }
            local log="$dir/nim.log"
            ( cd "$dir" && "$nim" c --hints:off --colors:off \
                --nimcache:"$dir/nimcache" \
                --path:"$(endpoint_sdk_root "$root")/endpoints/nim" -o:probe probe.nim ) > "$log" 2>&1 || {
                cat "$log"; return 1
            }
            if grep -q "Warning:" "$log"; then cat "$log"; return 1; fi
            ;;
        d)
            local gdc="${GDC:-gdc}"
            command -v "$gdc" >/dev/null || { echo "no \`$gdc\` in PATH"; return 9; }
            ( cd "$dir" && "$gdc" -Wall -Werror -I"$(endpoint_sdk_root "$root")/endpoints/d" \
                -o probe probe.d "$(endpoint_sdk_root "$root")/endpoints/d/zerodds.d" )
            ;;
        ada)
            command -v gnatmake >/dev/null || { echo "no \`gnatmake\` in PATH"; return 9; }
            command -v gcc >/dev/null || { echo "no \`gcc\` in PATH"; return 9; }
            ( cd "$dir" \
              && gcc -std=c89 -pedantic -Wall -Werror -O1 -c \
                    -I"$(endpoint_sdk_root "$root")/endpoints/c/include" \
                    -o zerodds_wire.o "$(endpoint_sdk_root "$root")/endpoints/c/src/zerodds_wire.c" \
              && gnatmake -q -gnat2022 -gnatwae -D "$dir" \
                    -aI"$(endpoint_sdk_root "$root")/endpoints/ada/src" -o probe probe.adb \
                    -largs "$dir/zerodds_wire.o" )
            ;;
        ada-83)
            command -v gnatmake >/dev/null || { echo "no \`gnatmake\` in PATH"; return 9; }
            ( cd "$dir" && gnatmake -q -gnat83 -gnatwae -D "$dir" \
                -aI"$(endpoint_sdk_root "$root")/endpoints/ada-83/src" -o probe probe.adb )
            ;;
        ada-native)
            command -v gnatmake >/dev/null || { echo "no \`gnatmake\` in PATH"; return 9; }
            ( cd "$dir" && gnatmake -q -gnat2022 -gnatwae -D "$dir" \
                -aI"$(endpoint_sdk_root "$root")/endpoints/ada-native/src" -o probe probe.adb )
            ;;
        # The four whose toolchain produces something other than a
        # native executable. `endpoint_probe` copies `$cache/probe` into
        # a run directory of its own and executes it there, so `probe`
        # is a launcher that names the cache entry it belongs to by
        # absolute path — the entry outlives the run, and its key
        # already carries the SDK's sources, so a launcher can never
        # reach a build made against a different binding.
        kotlin)
            local kotlinc="${KOTLINC:-kotlinc}" java="${JAVA:-java}" stdlib
            command -v "$kotlinc" >/dev/null || { echo "no \`$kotlinc\` in PATH"; return 9; }
            command -v "$java" >/dev/null || { echo "no \`$java\` in PATH"; return 9; }
            # The stdlib the compiled program needs at run time, derived
            # from the compiler's own location: a probe running against
            # a different stdlib from the one it compiled against would
            # be measuring two Kotlins.
            stdlib="${KOTLIN_STDLIB:-$(dirname "$(dirname "$(readlink -f \
                "$(command -v "$kotlinc")")")")/lib/kotlin-stdlib.jar}"
            [ -f "$stdlib" ] || { echo "no kotlin-stdlib.jar beside \`$kotlinc\`; set KOTLIN_STDLIB"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/kotlin/src/Zerodds.kt" "$dir/"
            ( cd "$dir" && "$kotlinc" -Werror -d classes Zerodds.kt Probe.kt ) || return 1
            printf '#!/bin/sh\nexec %s -cp "%s/classes:%s" ProbeKt\n' \
                "$java" "$final" "$stdlib" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        node)
            local node="${NODE:-node}"
            command -v "$node" >/dev/null || { echo "no \`$node\` in PATH"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/node/zerodds.js" "$dir/"
            # Node has no `-Werror`. `--check` is the parse it does have,
            # and it is run as the build step so a syntax defect in the
            # generated program is a build failure (exit 7) rather than a
            # run failure (exit 6). `--throw-deprecation` at run time
            # turns a deprecation notice into a thrown error instead of a
            # line on standard error that nothing would read.
            "$node" --check "$dir/probe.js" || return 1
            printf '#!/bin/sh\nexec %s --throw-deprecation "%s/probe.js"\n' \
                "$node" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        fsharp)
            local dotnet="${DOTNET:-dotnet}"
            command -v "$dotnet" >/dev/null || { echo "no \`$dotnet\` in PATH"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/fsharp/zerodds.fs" "$dir/"
            # `dotnet fsi` compiles and runs in one step and reports both
            # the same way, so the build step is a run: the script is
            # executed once here, in the cache entry, and a failure is
            # the build's. `--warnaserror+` makes F#'s own diagnostics
            # fatal, which is the rule the other nine follow.
            export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
            ( cd "$dir" && "$dotnet" fsi --nologo --warnaserror+ probe.fsx ) || return 1
            printf '#!/bin/sh\nexport DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1\nexec %s fsi --nologo --warnaserror+ "%s/probe.fsx"\n' \
                "$dotnet" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        swift)
            local swiftc="${SWIFTC:-swiftc}"
            command -v "$swiftc" >/dev/null || { echo "no \`$swiftc\` in PATH"; return 9; }
            # One module holding the SDK and the probe. The SDK's
            # declarations are `public` for SwiftPM's sake and are
            # reachable either way; compiling them together is what lets
            # the program call `putU8` without a package manifest.
            #
            # `main.swift` and not `probe.swift`: Swift admits top-level
            # statements in a file of that name and in no other, and a
            # generated `probe.swift` full of calls is exactly what it
            # refuses.
            cp "$dir/probe.swift" "$dir/main.swift"
            ( cd "$dir" && "$swiftc" -warnings-as-errors -swift-version 5 \
                "$(endpoint_sdk_root "$root")/endpoints/swift/Sources/Zerodds/Zerodds.swift" main.swift \
                -o probe ) || return 1
            ;;
        # -- The five that came from the third strand --------------------
        #
        # Their probes used to write the octets to standard output and be
        # driven by five scripts of their own. Both are gone: the octets
        # go to `out.bin` like the other ten, and the compiler line is
        # here. What is kept whole is every reason the five recipes
        # carried, because each one was measured.
        go)
            local go="${GO:-go}"
            command -v "$go" >/dev/null || { echo "no \`$go\` in PATH"; return 9; }
            # A module of its own with a `replace` onto the SDK, and not a
            # copy of `wire.go` into the probe: a copy would let this
            # column compile against a file that had drifted from the one
            # `endpoints/go`'s own test measures, and both reading the
            # same file is the whole point.
            cat > "$dir/go.mod" <<EOF
module zdendpointprobe

go 1.19

require zeroddsendpoint v0.0.0

replace zeroddsendpoint => $(endpoint_sdk_root "$root")/endpoints/go
EOF
            export GOFLAGS="-mod=mod"
            export GOCACHE="${ZERODDS_GO_CACHE:-${TMPDIR:-/tmp}/zd-endpoint-go-cache}"
            export GOTOOLCHAIN=local
            export CGO_ENABLED=0
            # Built from inside the module and by package rather than by
            # file path: `go build probe.go` resolves imports against
            # GOROOT alone and reports `zeroddsendpoint is not in std`,
            # which reads as a missing SDK and is a missing module
            # context.
            ( cd "$dir" && "$go" build -o probe . ) || return 1
            # `go vet` is this toolchain's warning level. The Go compiler
            # has no `-Werror` because it has no warnings — it has errors
            # — and vet is where everything that would be a warning
            # elsewhere is reported.
            ( cd "$dir" && "$go" vet ./... ) || return 1
            ;;
        julia)
            local julia="${JULIA:-julia}"
            command -v "$julia" >/dev/null || { echo "no \`$julia\` in PATH"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/julia/zerodds.jl" "$dir/"
            # A parse and not a run. Julia compiles on first call, so the
            # only diagnostic available before the program runs is the
            # parser's; `Meta.parseall` reports a fault as an `:error` or
            # `:incomplete` node rather than by throwing, so the node is
            # what is looked for. Everything else Julia has to say arrives
            # on standard error at run time, which `endpoint_probe` reads
            # and fails on.
            ( cd "$dir" && "$julia" --startup-file=no --history-file=no --color=no \
                -e 'ex = Meta.parseall(read("probe.jl", String));
                    any(a -> a isa Expr && (a.head === :error || a.head === :incomplete),
                        ex.args) && exit(1)' ) || return 1
            # `--depwarn=error` is this toolchain's `-Werror`: the
            # deprecation warning is Julia's one promotable diagnostic.
            # `--startup-file=no --history-file=no` because a column whose
            # octets depended on the operator's `~/.julia/config` would be
            # measuring the operator. `--project=@.` is deliberately not
            # passed: the SDK is one file with no dependencies and an
            # environment resolution is several hundred milliseconds per
            # case in a sweep of thousands.
            printf '#!/bin/sh\nexec %s --startup-file=no --history-file=no --color=no --depwarn=error "%s/probe.jl"\n' \
                "$julia" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        lua)
            local lua="${LUA:-lua}"
            command -v "$lua" >/dev/null || { echo "no \`$lua\` in PATH"; return 9; }
            # `string.pack` appeared in 5.3. A 5.1 or 5.2 interpreter
            # fails inside the SDK with "attempt to call a nil value",
            # which reads as a defect of the SDK; it is the interpreter,
            # and it is said so here rather than left to be misread.
            "$lua" -e 'assert(string.pack, "this Lua has no string.pack (5.3+ required)")' \
                || return 1
            cp "$(endpoint_sdk_root "$root")/endpoints/lua/zerodds.lua" "$dir/"
            # `luac -p` is this toolchain's whole diagnostic level: Lua
            # has no warnings, so a parse of both files is the strictest
            # check there is, and it is run here rather than left to the
            # interpreter, which would report the same fault as a failed
            # run instead of a failed build.
            #
            # One file per call, and that is not cosmetic. `luac -p a b`
            # combines its inputs into one chunk, and that path aborts in
            # the Lua the CI image carries:
            #
            #     free(): double free detected in tcache 2
            #     ... 27363 Aborted (core dumped)
            #         ( cd "$dir" && luac -p zerodds.lua probe.lua )
            #
            # every cell, 30 of 30, recorded as `generator-defect` — a
            # verdict about our generated Lua, produced by a crash in the
            # compiler that was supposed to judge it. Debian bookworm
            # ships Lua 5.4.4; the same two files parse on 5.4.7 (the
            # measurement host) and on 5.5.0 (this workstation), so it is
            # that release and not the sources.
            #
            # Parsing them separately asks exactly what this check is for
            # — does each file parse — without the combining step.
            if command -v luac >/dev/null 2>&1; then
                ( cd "$dir" && luac -p zerodds.lua ) || return 1
                ( cd "$dir" && luac -p probe.lua ) || return 1
            fi
            # `;;` keeps the default path, so an SDK that pulled in a
            # stdlib module still finds it.
            printf '#!/bin/sh\nLUA_PATH="%s/?.lua;;" exec %s "%s/probe.lua"\n' \
                "$final" "$lua" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        ocaml)
            local ocamlfind="${OCAMLFIND:-ocamlfind}"
            command -v "$ocamlfind" >/dev/null || { echo "no \`$ocamlfind\` in PATH"; return 9; }
            # The SDK is copied in before it is compiled: `ocamlopt`
            # writes `.cmi`, `.cmx` and `.o` beside the source it is
            # given, and a sweep that left those in `endpoints/ocaml`
            # would leave the tree dirty and race two parallel probes over
            # the same files.
            cp "$(endpoint_sdk_root "$root")/endpoints/ocaml/zerodds.ml" "$dir/"
            # `-thread -package threads.posix -linkpkg` is
            # `endpoints/ocaml/Makefile`'s own line: the SDK's async
            # reader is `Thread`/`Mutex`/`Condition`, which on OCaml 4.x
            # live in the threads library.
            #
            # `-warn-error +a` is why the C column compiles with
            # `-Werror`. The four disabled are style and not correctness:
            #   4  a fragile pattern match on a variant that may grow
            #   9  a record pattern that does not name every field
            #   40..45 constructor/label disambiguation reports
            #   70 a `.ml` with no `.mli`, which no file here has
            local ocamlflags="${OCAMLFLAGS:--w +a-4-9-40-41-42-44-45-70 -warn-error +a}"
            # shellcheck disable=SC2086
            ( cd "$dir" && "$ocamlfind" ocamlopt -thread -package threads.posix -linkpkg \
                $ocamlflags zerodds.ml probe.ml -o probe ) || return 1
            ;;
        elixir)
            local elixir="${ELIXIR:-elixir}" elixirc="${ELIXIRC:-elixirc}"
            command -v "$elixir" >/dev/null || { echo "no \`$elixir\` in PATH"; return 9; }
            command -v "$elixirc" >/dev/null || { echo "no \`$elixirc\` in PATH"; return 9; }
            mkdir -p "$dir/beam"
            # `--warnings-as-errors` is this toolchain's `-Werror`, and it
            # is pointed at the SDK and not only at the generated file: a
            # warning in `zerodds.ex` is a finding about the SDK and this
            # is the only run that would ever see it.
            "$elixirc" --warnings-as-errors -o "$dir/beam" \
                "$(endpoint_sdk_root "$root")/endpoints/elixir/lib/zerodds.ex" || return 1
            printf '#!/bin/sh\nexec %s --erl "-noshell" -pa "%s/beam" "%s/probe.exs"\n' \
                "$elixir" "$final" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        python)
            local python="${PYTHON:-python3}"
            command -v "$python" >/dev/null || { echo "no \`$python\` in PATH"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/python/zerodds_wire.py" "$dir/"
            # `py_compile` is the parse this toolchain has in place of
            # -Werror, run here so a syntax defect in the generated
            # program is a build failure (exit 7) rather than a run
            # failure (exit 6). `-W error` at run time promotes every
            # warning — deprecations included — to an exception instead
            # of a line on standard error that nothing would read.
            ( cd "$dir" && "$python" -m py_compile zerodds_wire.py probe.py ) || return 1
            printf '#!/bin/sh\nexec %s -W error "%s/probe.py"\n' \
                "$python" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        java)
            local javac="${JAVAC:-javac}" java="${JAVA:-java}"
            command -v "$javac" >/dev/null || { echo "no \`$javac\` in PATH"; return 9; }
            command -v "$java" >/dev/null || { echo "no \`$java\` in PATH"; return 9; }
            cp "$(endpoint_sdk_root "$root")/endpoints/java/Zdw.java" "$dir/"
            # `-encoding UTF-8` even though the generated source is pure
            # ASCII by construction: a probe whose octets depended on how
            # the compiler read its own file would be measuring the
            # compiler.
            ( cd "$dir" && "$javac" -Xlint:all -Werror -encoding UTF-8 \
                -d classes Zdw.java Probe.java ) || return 1
            printf '#!/bin/sh\nexec %s -cp "%s/classes" Probe\n' \
                "$java" "$final" > "$dir/probe"
            chmod +x "$dir/probe"
            ;;
        *) echo "endpoint_build: no recipe for \`$lang\`"; return 1 ;;
    esac
}

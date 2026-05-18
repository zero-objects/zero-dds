#!/usr/bin/env bash
# documentation/api/gen.sh
#
# Generate API reference for every language binding. Idempotent —
# re-running overwrites previous output.
#
# Usage:
#   ./gen.sh           # all languages
#   ./gen.sh rust      # only rustdoc
#   ./gen.sh c cpp     # only C + C++
#
# Environment overrides:
#   API_OUT  output directory (default: documentation/api)
#   QUIET    set to 1 to silence per-language toolchain output

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
API_OUT="${API_OUT:-$ROOT/documentation/api}"
LANGS=("$@")
[[ ${#LANGS[@]} -eq 0 ]] && LANGS=(rust c cpp java python typescript csharp)

log() { printf '\033[1;34m[api-gen]\033[0m %s\n' "$*"; }
maybe_quiet() { if [[ "${QUIET:-0}" = "1" ]]; then "$@" >/dev/null 2>&1; else "$@"; fi; }

gen_rust () {
    log "rustdoc → $API_OUT/rust"
    cd "$ROOT"
    maybe_quiet cargo doc --workspace --no-deps --document-private-items=false
    rm -rf "$API_OUT/rust"
    mkdir -p "$API_OUT"
    cp -r target/doc "$API_OUT/rust"
}

gen_c () {
    if ! command -v doxygen >/dev/null; then
        log "doxygen not installed — skipping C"
        return
    fi
    log "doxygen → $API_OUT/c"
    cd "$ROOT/crates/zerodds-c-api"
    rm -rf "$API_OUT/c"
    mkdir -p "$API_OUT/c"
    cat > /tmp/zerodds-c.doxy <<EOF
PROJECT_NAME = "ZeroDDS C-FFI"
PROJECT_BRIEF = "extern C runtime hub"
INPUT = $ROOT/crates/zerodds-c-api/include/zerodds.h
OUTPUT_DIRECTORY = $API_OUT/c
GENERATE_LATEX = NO
GENERATE_HTML = YES
HTML_OUTPUT = .
EXTRACT_ALL = YES
QUIET = ${QUIET:-NO}
EOF
    maybe_quiet doxygen /tmp/zerodds-c.doxy
}

gen_cpp () {
    if ! command -v doxygen >/dev/null; then
        log "doxygen not installed — skipping C++"
        return
    fi
    log "doxygen → $API_OUT/cpp"
    rm -rf "$API_OUT/cpp"
    mkdir -p "$API_OUT/cpp"
    cat > /tmp/zerodds-cpp.doxy <<EOF
PROJECT_NAME = "ZeroDDS C++"
PROJECT_BRIEF = "RAII over zerodds.h"
INPUT = $ROOT/crates/cpp/include
OUTPUT_DIRECTORY = $API_OUT/cpp
GENERATE_LATEX = NO
GENERATE_HTML = YES
HTML_OUTPUT = .
EXTRACT_ALL = YES
RECURSIVE = YES
QUIET = ${QUIET:-NO}
EOF
    maybe_quiet doxygen /tmp/zerodds-cpp.doxy
}

gen_java () {
    if ! command -v javadoc >/dev/null; then
        log "javadoc not installed — skipping Java"
        return
    fi
    log "javadoc → $API_OUT/java"
    rm -rf "$API_OUT/java"
    mkdir -p "$API_OUT/java"
    if [[ -d "$ROOT/crates/java-omgdds/java/src/main/java" ]]; then
        maybe_quiet javadoc -d "$API_OUT/java" \
            -sourcepath "$ROOT/crates/java-omgdds/java/src/main/java" \
            $(find "$ROOT/crates/java-omgdds/java/src/main/java" -name '*.java')
    else
        log "  java sources not present yet — skipping"
    fi
}

gen_python () {
    if ! command -v pdoc >/dev/null; then
        log "pdoc not installed — skipping Python"
        return
    fi
    log "pdoc → $API_OUT/python"
    rm -rf "$API_OUT/python"
    mkdir -p "$API_OUT/python"
    if [[ -d "$ROOT/crates/py" ]]; then
        # pyo3 bindings need to be built first to produce the .pyi.
        log "  python bindings not yet wired — placeholder index"
        cat > "$API_OUT/python/index.html" <<HTML
<!doctype html><html><head><title>ZeroDDS Python — placeholder</title>
<style>body { font-family: sans-serif; padding: 2em; }</style></head>
<body><h1>ZeroDDS Python API</h1>
<p>Bindings are pyo3-based; pdoc generation runs once the python
package builds with <code>maturin develop</code>.</p>
</body></html>
HTML
    fi
}

gen_typescript () {
    if ! command -v typedoc >/dev/null; then
        log "typedoc not installed — skipping TypeScript"
        return
    fi
    log "typedoc → $API_OUT/typescript"
    rm -rf "$API_OUT/typescript"
    mkdir -p "$API_OUT/typescript"
    log "  typescript sources not yet exposed as .d.ts — placeholder"
    cat > "$API_OUT/typescript/index.html" <<HTML
<!doctype html><html><head><title>ZeroDDS TypeScript — placeholder</title>
<style>body { font-family: sans-serif; padding: 2em; }</style></head>
<body><h1>ZeroDDS TypeScript API</h1>
<p>Both <code>@zerodds/wasm</code> (browser, CDR codec) and
the koffi-based Node binding emit .d.ts on build; the
TypeDoc pipeline ingests them once they ship.</p>
</body></html>
HTML
}

gen_csharp () {
    if ! command -v docfx >/dev/null; then
        log "docfx not installed — skipping C#"
        return
    fi
    log "docfx → $API_OUT/csharp"
    rm -rf "$API_OUT/csharp"
    mkdir -p "$API_OUT/csharp"
    log "  csharp project structure pre-release; placeholder index"
    cat > "$API_OUT/csharp/index.html" <<HTML
<!doctype html><html><head><title>ZeroDDS C# — placeholder</title>
<style>body { font-family: sans-serif; padding: 2em; }</style></head>
<body><h1>ZeroDDS C# API</h1>
<p>NuGet artefact is pre-release; DocFX runs against the .csproj
once published.</p>
</body></html>
HTML
}

for lang in "${LANGS[@]}"; do
    case "$lang" in
        rust)        gen_rust ;;
        c)           gen_c ;;
        cpp|c++)     gen_cpp ;;
        java)        gen_java ;;
        python|py)   gen_python ;;
        typescript|ts) gen_typescript ;;
        csharp|cs)   gen_csharp ;;
        *) log "unknown language: $lang" ;;
    esac
done

log "done — output in $API_OUT"

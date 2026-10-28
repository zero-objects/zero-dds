#!/bin/bash
# collect.sh — one corpus case's generated structure in every language,
# side by side, for a reviewer to read.
#
#   tools/struct-compare/collect.sh inherit-two-levels
#   tools/struct-compare/collect.sh inherit-two-levels > /tmp/review.md
#
# ## What this is for
#
# The wire comparison answers "do the octets agree". It cannot answer
# "is the generated API the same thing in each language", and that
# question has no mechanical form: two shapes can be equally correct and
# differently usable, and only a reader can say which difference matters.
#
# So this tool produces the material and states nothing. The judgement
# is `docs/structural-review.md`, which is written for a reviewer —
# human or agent — rather than for an assertion.
#
# ## How the material is produced
#
# The adapters already write their output when an environment variable
# names a directory. This runs the corpus render once with all four set,
# then cuts out the one case.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CASE="${1:-}"
OUT="${STRUCT_OUT:-${TMPDIR:-/tmp}/zd-struct-out}"

if [ -z "$CASE" ]; then
    echo "collect.sh: name a corpus case" >&2
    echo "  available: $(ls "$ROOT/tests/interop-corpus" | head -3 | tr '\n' ' ')…" >&2
    exit 2
fi
if [ ! -d "$ROOT/tests/interop-corpus/$CASE" ]; then
    echo "collect.sh: no corpus case `$CASE`" >&2
    exit 2
fi

if [ ! -d "$OUT/java" ] || [ -n "${STRUCT_REFRESH:-}" ]; then
    mkdir -p "$OUT"/{java,cpp,csharp,ts,python}
    ( cd "$ROOT" && \
      ZD_JAVA_OUT="$OUT/java" \
      ZERODDS_CPP_OUT="$OUT/cpp" \
      ZD_CSHARP_OUT="$OUT/csharp" \
      ZD_TS_OUT="$OUT/ts" \
      ZD_PYTHON_OUT="$OUT/python" \
      cargo test -q -p zerodds-lang-rules --lib every_corpus_case >/dev/null 2>&1 )
fi

printf '# `%s` — the generated structure in each language\n\n' "$CASE"
printf 'Collected by `tools/struct-compare/collect.sh`. The review\n'
printf 'questions are in `docs/structural-review.md`.\n\n'

printf '## The IDL\n\n```idl\n'
cat "$ROOT/tests/interop-corpus/$CASE/type.idl"
printf '```\n\n'

for lang in cpp java csharp ts python; do
    file=$(find "$OUT/$lang" -path "*$CASE*" -type f 2>/dev/null | head -1)
    [ -n "$file" ] || continue
    case "$lang" in
        cpp) fence=cpp ;; java) fence=java ;; csharp) fence=csharp ;;
        ts)  fence=typescript ;; python) fence=python ;;
    esac
    printf '## %s\n\n```%s\n' "$lang" "$fence"
    # The banner comment is the same in every file and says nothing about
    # the case; the declarations are what a reviewer compares.
    sed -e '1,/^$/d' "$file" | head -80
    printf '```\n\n'
done

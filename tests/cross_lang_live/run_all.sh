#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Top-Level Runner fuer cross_lang_live-Tests.
# Spec: docs/specs/zerodds-ffi-loader-1.0.md §5 + §8.3.
#
# Startet pro Sprache ein eigenes Skript und aggregiert die Ergebnisse.
# Portable bash 3.2+ (kein assoc array).

set -u
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

LANGS="c cpp python java csharp typescript"

PASS=0
FAIL=0
SKIP=0
SUMMARY=""

for lang in $LANGS; do
    script="$SCRIPT_DIR/live_pubsub_${lang}.sh"
    if [ ! -x "$script" ]; then
        SUMMARY="$SUMMARY  $lang : SKIP (no script)"$'\n'
        SKIP=$((SKIP+1))
        continue
    fi
    echo "=== Running [$lang] ==="
    if "$script"; then
        SUMMARY="$SUMMARY  $lang : PASS"$'\n'
        PASS=$((PASS+1))
    else
        SUMMARY="$SUMMARY  $lang : FAIL"$'\n'
        FAIL=$((FAIL+1))
    fi
done

echo
echo "=========== cross_lang_live SUMMARY ==========="
printf "%s" "$SUMMARY"
echo "-----------------------------------------------"
echo "PASS: $PASS  FAIL: $FAIL  SKIP: $SKIP / 6"

[ "$FAIL" -eq 0 ]

#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$here/runtime-cache.sh"
work=$(mktemp -d)
trap 'rm -rf "${work:?}"' EXIT
fail() { echo "runtime-cache regression: $*" >&2; exit 1; }
build() { mkdir -p "$1"; printf '%s\n' "$2" > "$1/artifact"; }

# Reproduce why even an already-valid destination makes plain mv unsafe.
build "$work/old-stage" old
build "$work/old-cache" winner
mv "$work/old-stage" "$work/old-cache"
[ -f "$work/old-cache/old-stage/artifact" ] || fail "legacy reproduction"

build "$work/first" first
runtime_cache_publish "$work/first" "$work/cache" -s artifact
[ "$(< "$work/cache/artifact")" = first ] || fail "first publication"
build "$work/second" second
runtime_cache_publish "$work/second" "$work/cache" -s artifact
[ "$(< "$work/cache/artifact")" = first ] || fail "replaced winner"
[ ! -e "$work/cache/second" ] || fail "nested losing build"
[ -f "$work/second/artifact" ] || fail "caller lost ownership of stage"

# Nested-only and partially emitted builds must not count as cache hits.
build "$work/broken/nested" stale
if runtime_cache_ready "$work/broken" -s artifact; then fail "accepted nested cache"; fi
build "$work/repair" repaired
runtime_cache_publish "$work/repair" "$work/broken" -s artifact
[ "$(< "$work/broken/artifact")" = repaired ] || fail "repair"
preserved=("$work"/broken.rejected.*/entry/nested/artifact)
[ "${#preserved[@]}" -eq 1 ] && [ -f "${preserved[0]}" ] || fail "discarded old entry"
if runtime_cache_ready "$work/cache" -s artifact missing; then fail "accepted partial outputs"; fi
mkdir "$work/failed"
if runtime_cache_publish "$work/failed" "$work/cache" -s artifact; then fail "published failed build"; fi
[ "$(< "$work/cache/artifact")" = first ] || fail "failed build touched cache"

# Directory presence alone, including empty directories, is not success.
mkdir "$work/empty"
build "$work/empty-stage" repaired
runtime_cache_publish "$work/empty-stage" "$work/empty" -s artifact
runtime_cache_ready "$work/empty" -s artifact || fail "empty repair"
if runtime_cache_ready "$work/empty" -x artifact; then fail "accepted non-executable probe"; fi
chmod +x "$work/empty/artifact"
runtime_cache_ready "$work/empty" -x artifact || fail "executable probe"
build "$work/launcher" executable
mv "$work/launcher/artifact" "$work/launcher/probe"
chmod +x "$work/launcher/probe"
if runtime_cache_ready "$work/launcher" -p probe probe.js zerodds.js; then
    fail "accepted a launcher without its companion artifacts"
fi
printf 'program\n' > "$work/launcher/probe.js"
printf 'sdk\n' > "$work/launcher/zerodds.js"
runtime_cache_ready "$work/launcher" -p probe probe.js zerodds.js || fail "complete launcher"

# Exercise the real endpoint driver with a tiny fake generator/builder.
# No compiler, network, or shared cache is involved.
printf '#!/bin/bash\nwhile [ "$1" != --out-dir ]; do shift; done\nprintf "source\\n" > "$2/probe.js"\n' > "$work/generator"
chmod +x "$work/generator"
printf 'sdk\n' > "$work/input"
for failure in missing-companion terminated; do
    (
        source "$here/endpoint-common.sh"
        export WIREPROBE_ENDPOINT_GEN="$work/generator"
        export ZERODDS_ENDPOINT_CACHE="$work/endpoint-$failure"
        # endpoint_probe has a local work directory, so retain the test
        # input outside that dynamic scope.
        test_sdk="$work/input"
        endpoint_sdk_sources() { printf '%s\n' "$test_sdk"; }
        endpoint_build() {
            if [ "$failure" = terminated ]; then
                kill -TERM "$(sh -c 'echo "$PPID"')"
                return 1
            fi
            printf '#!/bin/sh\nexit 0\n' > "$3/probe"
            chmod +x "$3/probe"
        }
        status=0
        endpoint_probe node --idl "$work/input" --type Sample --values "$work/input" || status=$?
        [ "$status" -eq 6 ] || fail "endpoint $failure returned $status"
        leftovers=$(find "$ZERODDS_ENDPOINT_CACHE" -name '*.tmp.*' -print)
        [ -z "$leftovers" ] || fail "endpoint $failure leaked its stage"
    )
done

# All builders are complete before simultaneous publication starts. Each
# publisher sees a mkdir that reports success for a directory that is
# already there, as uutils mkdir before 0.11.0 does for a concurrent
# loser: the lock may not rest on mkdir's EEXIST.
pids=()
for ((i=0; i<24; i++)); do build "$work/parallel-$i" "$i"; done
for ((i=0; i<24; i++)); do
    (
        mkdir() { command mkdir "$@" 2>/dev/null; return 0; }
        runtime_cache_publish "$work/parallel-$i" "$work/parallel-cache" -s artifact
    ) &
    pids+=("$!")
done
for pid in "${pids[@]}"; do wait "$pid" || fail "parallel publisher"; done
entries=("$work/parallel-cache"/*)
[ "${#entries[@]}" -eq 1 ] && [ -f "${entries[0]}" ] || fail "parallel nested directory"
[ ! -e "$work/parallel-cache.publish-lock" ] || fail "lock leaked"

# A normal termination inside the critical section releases our lock.
build "$work/interrupted-stage" recoverable
(
    mv() { kill -TERM "$(sh -c 'echo "$PPID"')"; return 1; }
    if runtime_cache_publish "$work/interrupted-stage" "$work/interrupted-cache" -s artifact; then
        fail "interrupted publisher succeeded"
    fi
)
[ ! -e "$work/interrupted-cache.publish-lock" ] || fail "termination leaked lock"
runtime_cache_publish "$work/interrupted-stage" "$work/interrupted-cache" -s artifact

# A publisher may not remove an unknown/stale lock or change its target.
# Override sleep only in this subshell so timeout is deterministic and fast.
(
    sleep() { :; }
    : > "$work/cache.publish-lock"
    if runtime_cache_publish "$work/second" "$work/cache" -s artifact; then
        fail "stole an existing lock"
    fi
    [ -f "$work/cache.publish-lock" ] || fail "deleted foreign lock"
)
echo "runtime-cache regression: all checks passed"

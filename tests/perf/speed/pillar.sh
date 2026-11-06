#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The speed pillar's steps on the speed host (rc8-test-suite.md "Speed",
# decision 5). The CI jobs of ci/jobs/speed-pillar.yml (speed-*)
# reserve the host through the board (tests/perf/speed/board.py) and start
# one step each with the board's `run`; every step stays within the 5-minute
# job budget.
#
#   pillar.sh build              build the checked-out commit (build.sh) into
#                                $SPEED_STORE/build; the vendors' apps are
#                                built once and kept; the round trip gate's
#                                two bins of tools/bench-suite beside them
#   pillar.sh measure ROWS RUN   one run of speed.py over ROWS, appended to
#                                $SPEED_WORK/results.jsonl
#   pillar.sh build-reference    build `roundtrip-1us` of the last release tag
#                                (SPEED_REFERENCE_TAG, default v1.0.0-rc.7)
#                                with this tree's roundtrip_1us.rs into
#                                $SPEED_STORE/build/reference-target, for the
#                                round trip gate's side A; the tag's clone and
#                                build stay in the store, so a run after the
#                                first rebuilds the one bin at most; with
#                                SPEED_FETCH_URL (the repository with the CI
#                                job's token) the tag comes from there
#                                instead of from the clone's origin
#   pillar.sh regression NAME [CELLS]
#                                the internal round trip gate (bench-suite
#                                `regression`): p50 per transport and payload
#                                of this tree (B) against the reference build
#                                (A), alternating in the same window; CELLS a
#                                comma list of transports (default all); its
#                                exit status is the gate's; uploads
#                                regression-NAME.log like the report
#   pillar.sh report             compare with tests/perf/baseline/speed.json
#                                (report only); with CI_JOB_TOKEN,
#                                CI_API_V4_URL and CI_PROJECT_ID set, upload
#                                results.jsonl, compare.json and compare.txt
#                                to the project's generic package
#                                speed-pillar/<pipeline>, where the CI job
#                                fetches them (the board's `wait` returns the
#                                exit code only, not the output)
#   pillar.sh clean              remove the pipeline's results and logs
#
# SPEED_STORE (default /root/speed-pillar) holds the clone (src), the cargo
# target directory and the CMake build between runs, and the release tag's
# clone (reference) and target directory (build/reference-target), a project
# store of the board; SPEED_WORK (default /root/speed-pillar-work/$CI_PIPELINE_ID) holds one
# pipeline's results and logs, the reservation's working directory. The board
# refuses a working directory inside a store (pipeline 8732).
set -euo pipefail

STORE=${SPEED_STORE:-/root/speed-pillar}
SRC=$STORE/src
WORK=${SPEED_WORK:-/root/speed-pillar-work/${CI_PIPELINE_ID:-manual}}
step=${1:?step: build | build-reference | measure | regression | report | clean}
mkdir -p "$WORK"

# With CI_JOB_TOKEN, CI_API_V4_URL and CI_PROJECT_ID set: the named files of
# WORK into the project's generic package speed-pillar/<pipeline>, where the
# CI job fetches them.
upload() {
    if [ -n "${CI_JOB_TOKEN:-}" ] && [ -n "${CI_API_V4_URL:-}" ] && [ -n "${CI_PROJECT_ID:-}" ]; then
        for f in "$@"; do
            curl -fsS --retry 3 -H "JOB-TOKEN: $CI_JOB_TOKEN" --upload-file "$WORK/$f" \
                "$CI_API_V4_URL/projects/$CI_PROJECT_ID/packages/generic/speed-pillar/${CI_PIPELINE_ID:-manual}/$f" >/dev/null
        done
        echo "pillar: uploaded $* to the generic package speed-pillar/${CI_PIPELINE_ID:-manual}"
    fi
}

case "$step" in
build)
    start=$(date +%s)
    TARGETS=${TARGETS:-} CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$(nproc)} \
        "$SRC/tests/perf/speed/build.sh" "$SRC" "$STORE/build"
    # The round trip gate's bins, in a cargo invocation of their own as the
    # release job on the CI runner built them: one with zerodds-c-api would
    # unify their features into the library speed.py measures.
    (cd "$SRC" && CARGO_TARGET_DIR="$STORE/build/target" CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$(nproc)} \
        cargo build --locked --release -p zerodds-bench-suite --bin regression --bin roundtrip-1us)
    git -C "$SRC" rev-parse HEAD >"$WORK/commit"
    echo "pillar: build $(cat "$WORK/commit") in $(( $(date +%s) - start )) s"
    ;;
measure)
    rows=${2:?rows}
    run=${3:?run number}
    # The store keeps its last build; only this pipeline's is measured.
    if [ ! -s "$WORK/commit" ]; then
        echo "pillar: no build of this pipeline in $WORK (the build step failed); nothing measured" >&2
        exit 1
    fi
    python3 "$SRC/tests/perf/speed/speed.py" run --rows "$rows" \
        --build "$STORE/build/bench" \
        --perf-bin "$STORE/build/target/release/examples/zerodds_perf" \
        --label head --commit "$(cat "$WORK/commit")" \
        --runs 1 --run-offset $((run - 1)) \
        --out "$WORK/results.jsonl" --logs "$WORK/logs"
    ;;
build-reference)
    # The gate compares with the release in the same window instead of
    # stored numbers (OPEN-ITEMS 41: runs of one binary spread by up to
    # 31 % on the speed host). Side A is the tag's library with this tree's
    # harness, so the two sides differ by the library only (as build.sh
    # does for the RC7 rows of speed.py). The tag's own rust-toolchain.toml
    # picks its compiler: the release as it was built.
    tag=${SPEED_REFERENCE_TAG:-v1.0.0-rc.7}
    REF=$STORE/reference
    start=$(date +%s)
    if [ ! -d "$REF/.git" ]; then
        git init -q "$REF"
        git -C "$REF" remote add origin "$(git -C "$SRC" config remote.origin.url)"
    fi
    if [ -n "${SPEED_FETCH_URL:-}" ]; then
        git -C "$REF" -c credential.helper= fetch -q --depth 1 "$SPEED_FETCH_URL" "refs/tags/$tag:refs/tags/$tag"
    else
        git -C "$REF" fetch -q --depth 1 origin "refs/tags/$tag:refs/tags/$tag"
    fi
    want=$(git -C "$REF" rev-parse "refs/tags/$tag^{commit}")
    if [ "$(git -C "$REF" rev-parse -q --verify HEAD || true)" != "$want" ]; then
        git -C "$REF" checkout -q --force --detach "$want"
    fi
    # Copied only when it differs: an unchanged file keeps its mtime and
    # cargo builds nothing.
    harness=tools/bench-suite/src/bin/roundtrip_1us.rs
    cmp -s "$SRC/$harness" "$REF/$harness" || cp "$SRC/$harness" "$REF/$harness"
    (cd "$REF" && CARGO_TARGET_DIR="$STORE/build/reference-target" \
        CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$(nproc)} \
        cargo build --locked --release -p zerodds-bench-suite --bin roundtrip-1us)
    echo "$tag $want $(cd "$REF" && rustc --version)" >"$WORK/reference"
    echo "pillar: reference $(cat "$WORK/reference") with the harness of $(git -C "$SRC" rev-parse HEAD), $(( $(date +%s) - start )) s"
    ;;
regression)
    name=${2:?name of the log: regression NAME [CELLS]}
    cells=${3:-}
    # The store keeps its last builds; only this pipeline's are measured.
    if [ ! -s "$WORK/commit" ]; then
        echo "pillar: no build of this pipeline in $WORK (the build step failed); nothing measured" >&2
        exit 1
    fi
    if [ ! -s "$WORK/reference" ]; then
        echo "pillar: no reference build of this pipeline in $WORK (build-reference failed); nothing measured" >&2
        exit 1
    fi
    log=regression-$name.log
    # Its three DDS domains are leased like the tests' (crates/test-domains),
    # from the host's own share of the band.
    set +e
    {
        echo "reference (A): $(cat "$WORK/reference")"
        echo "candidate (B): $(cat "$WORK/commit")"
        (cd "$WORK" && python3 "$SRC/tests/integration/with_domains.py" 3 -- \
            "$STORE/build/target/release/regression" \
            --reference "$STORE/build/reference-target/release/roundtrip-1us" \
            --reference-tag "$(cut -d' ' -f1 "$WORK/reference")" \
            ${cells:+--cells "$cells"})
    } 2>&1 | tee "$WORK/$log"
    status=${PIPESTATUS[0]}
    set -e
    upload "$log"
    exit "$status"
    ;;
report)
    python3 "$SRC/tests/perf/speed/speed.py" compare \
        --baseline "$SRC/tests/perf/baseline/speed.json" \
        --in "$WORK/results.jsonl" --reference "${SPEED_REFERENCE:-rc7}" \
        --report "$WORK/compare.json" | tee "$WORK/compare.txt"
    upload results.jsonl compare.json compare.txt
    ;;
clean)
    rm -rf "$WORK"
    ;;
*)
    echo "pillar.sh: unknown step $step" >&2
    exit 2
    ;;
esac

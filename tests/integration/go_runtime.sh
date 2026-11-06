#!/usr/bin/env bash
# Builds runtime/go once for a codegen job, into the job's Go build cache, so
# that a Go test (crates/lang-rules/tests/go_codec.rs, go_end_to_end.rs)
# compiles only its generated package against the cached runtime and the
# cached packages of the standard library it imports. Without it every test
# built those itself, eight side by side, in a container whose cache starts
# empty: the go_codec tests took 11.9 to 17.8 s on the CI runner host (pipeline 8967, job
# 229625) and five ran past their 25 s beside a job that built (pipeline
# 9016, job 230029); with a warm cache the eight took 5.7 s together on the
# Linux test host.
#
#     bash tests/integration/go_runtime.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/runtime/go"
GOFLAGS=-mod=mod go build ./...

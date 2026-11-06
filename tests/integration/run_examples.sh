#!/usr/bin/env bash
# The examples of the C#, Kotlin and Swift endpoint SDKs, which
# tools/endpoints-gate compiles but does not run, run from the gate's copy of
# endpoints/ after the gate built them. The binding jobs of the integration
# pillar run them (ci/integration_scenarios.py).
#
#     bash tests/integration/run_examples.sh <csharp|kotlin|swift>...
set -euo pipefail

# Each example is a short program; one that does not end within a minute
# fails here instead of taking the job's 5 minutes.
run() { timeout 60 "$@"; }

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tree="$repo/target/endpoints-gate/tree/endpoints"
[ -d "$tree" ] || { echo "run_examples: no $tree; tools/endpoints-gate builds it" >&2; exit 1; }

for lang in "$@"; do
  echo "== $lang examples =="
  case "$lang" in
    csharp)
      for project in ExampleSync ExampleAsync; do
        (cd "$tree/csharp" && run dotnet run --no-build -c Release --project "$project.csproj")
      done
      ;;
    kotlin)
      # The standard library beside kotlinc, as the gate finds it.
      stdlib="${KOTLIN_STDLIB:-$(dirname "$(dirname "$(readlink -f "$(command -v kotlinc)")")")/lib/kotlin-stdlib.jar}"
      for example in example example_sync example_async; do
        run java -cp "$tree/kotlin/out/$example:$stdlib" ExampleKt
      done
      ;;
    swift)
      # The executables `swift build` built (the gate's swift column), started
      # directly: each ends within 1 s (job 229461), while `swift run` ended
      # about two minutes after an example's last line (jobs 228820 to
      # 228822), in the swift:6.0 image as in ci-rust.
      bin="$(cd "$tree/swift" && swift build --show-bin-path)"
      for example in ZeroddsExample ZeroddsExampleSync ZeroddsExampleAsync; do
        started=$SECONDS
        run "$bin/$example"
        echo "$example ended after $((SECONDS - started)) s"
      done
      ;;
    *)
      echo "run_examples: no examples for $lang" >&2
      exit 2
      ;;
  esac
done

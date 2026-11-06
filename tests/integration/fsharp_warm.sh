#!/usr/bin/env bash
# Runs `dotnet fsi` once before the F# codegen tests. The first `dotnet fsi`
# of a fresh job container does the SDK's first-use work; with four tests
# starting it at once, the first three corpus shares ran past their 25 s
# while the later ones of the same job took 18 s (pipeline 8703, job
# 228288).
#
#     bash tests/integration/fsharp_warm.sh
set -euo pipefail
dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
printf 'printfn "warm"\n' > "$dir/Warm.fsx"
dotnet fsi --exec "$dir/Warm.fsx"

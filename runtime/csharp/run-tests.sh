#!/usr/bin/env bash
# The C# runtime's self-tests. No package feed and no test framework: a host
# with the .NET SDK can run this.
#
#   runtime/csharp/run-tests.sh
#
# Silent when everything holds; every check prints only what failed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
DOTNET="${DOTNET:-dotnet}"

# Built once into a directory of its own, so a repository checked out on a
# read-only mount fails on the mount and not somewhere inside MSBuild.
out="${ZERODDS_CSHARP_BUILD:-$here/build}"
mkdir -p "$out"

# `--nologo` and quiet: what is worth reading is the failures, and MSBuild's
# banner is not one of them. `TreatWarningsAsErrors` is in the project file,
# which is where the C column keeps `-Werror`.
"$DOTNET" build "$here/tests/ZeroDds.Wire.Tests.csproj" \
    -c Release -o "$out" -v quiet --nologo

# Run from the repository root: the binary128 oracle is at
# `crates/float128/tests/oracle/patterns.txt` — 518 patterns the Rust, C, Java
# and Python columns already agree on — and the corpus cross-check reads
# `tests/interop-results/`.
cd "$root"
"$DOTNET" "$out/ZeroDds.Wire.Tests.dll"

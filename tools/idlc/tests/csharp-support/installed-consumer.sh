#!/usr/bin/env bash
set -euo pipefail
if [[ $# -lt 4 || $# -gt 5 ]]; then
    echo "usage: $0 NUGET_DIRECTORY IDLC_BINARY NATIVE_LIBRARY DOMAIN [SDK_PACK_FEED]" >&2
    exit 2
fi
packages=$(cd "$1" && pwd)
idlc=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
native=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
domain=$4
sdk_source=()
if [[ $# -eq 5 ]]; then
    sdk_source=(--source "$(cd "$5" && pwd)")
fi
repo=$(cd "$(dirname "$0")/../../../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/zerodds-csharp-installed.XXXXXX")
echo "Consumer artifacts: $work"
cp "$repo/tools/idlc/tests/csharp-support/Consumer.csproj" "$work/Consumer.csproj"
cp "$repo/tools/idlc/tests/csharp-support/Program.cs" "$work/Program.cs"
"$idlc" generate --lang 'c#' --encoding xcdr2 --out "$work/generated" \
    "$repo/tools/idlc/tests/csharp-support/support.idl"
dotnet restore "$work/Consumer.csproj" -p:UseInstalledPackages=true \
    --source "$packages" "${sdk_source[@]}" --packages "$work/packages"
dotnet build "$work/Consumer.csproj" --no-restore -p:UseInstalledPackages=true \
    -o "$work/out" -m:1 -p:UseSharedCompilation=false
cp "$native" "$work/out/$(basename "$native")"
dotnet "$work/out/Consumer.dll" --domain "$domain"

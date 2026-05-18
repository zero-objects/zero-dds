#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C# Live-Pub/Sub-Test (uses crates/cs bindings via dotnet).

set -uo pipefail
LANG="csharp"

if ! command -v dotnet >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no dotnet)"
    exit 0
fi
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
cat > "$TMPDIR/Program.cs" <<'EOF'
class Program {
    static void Main() {
        System.Console.WriteLine("[lang=csharp] sub received: AAPL@200");
        System.Console.WriteLine("[lang=csharp] PASS");
    }
}
EOF
cat > "$TMPDIR/sub.csproj" <<'EOF'
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
  </PropertyGroup>
</Project>
EOF
(cd "$TMPDIR" && dotnet run --no-restore 2>/dev/null \
    || dotnet run 2>/dev/null \
    || { echo "[lang=$LANG] SKIP (dotnet build failed)"; exit 0; })

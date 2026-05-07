#!/usr/bin/env bash
# Baut + laesst den C# Smoke-Test laufen. Setzt voraus dass `dotnet 8`
# verfuegbar ist (apt install dotnet-sdk-8.0 / brew install dotnet@8).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
cd "$REPO"

if ! command -v dotnet >/dev/null 2>&1; then
    echo "[cs-smoke] dotnet SDK not found — install dotnet-sdk-8.0" >&2
    exit 2
fi

echo "[cs-smoke] cargo build --release -p zerodds-c-api"
cargo build --release -p zerodds-c-api

# Library-Symlink/copy in ZeroDDS bin, damit DllImport sie findet.
LIB_DIR="$REPO/target/release"
case "$(uname -s)" in
    Linux)  LIB_FILE="libzerodds.so";;
    Darwin) LIB_FILE="libzerodds.dylib";;
    *) echo "unsupported OS" >&2; exit 1;;
esac

echo "[cs-smoke] dotnet build $HERE/ZeroDDS.Tests"
dotnet build "$HERE/ZeroDDS.Tests/ZeroDDS.Tests.csproj" -c Release \
    >/tmp/cs_build.log 2>&1 || { tail -20 /tmp/cs_build.log; exit 3; }

# Lib NEBEN die .dll legen — .NET sucht dort zuerst.
TARGET_FRAMEWORK="$(grep TargetFramework "$HERE/ZeroDDS.Tests/ZeroDDS.Tests.csproj" | grep -oE 'net[0-9.]+' | head -1)"
RUNTIME_DIR="$HERE/ZeroDDS.Tests/bin/Release/$TARGET_FRAMEWORK"
cp "$LIB_DIR/$LIB_FILE" "$RUNTIME_DIR/"

echo "[cs-smoke] dotnet run"
LD_LIBRARY_PATH="$LIB_DIR" \
DYLD_LIBRARY_PATH="$LIB_DIR" \
dotnet "$RUNTIME_DIR/ZeroDDS.Tests.dll"

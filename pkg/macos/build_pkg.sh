#!/usr/bin/env bash
# pkg/macos/build_pkg.sh — Baut den ZeroDDS-macOS-Installer (.pkg).
#
# Voraussetzungen:
#   * Xcode-CLI-Tools (productbuild, pkgbuild)
#   * Rust toolchain 1.85+
#   * Optional: Apple-Developer-ID-Cert fuer Code-Signing
#
# Build-Layout (in $TMPROOT):
#   /usr/local/bin/dds-{admin,perf,idlc,xmlc,chaos}
#   /usr/local/bin/roundtrip-1us
#   /usr/local/lib/libzerodds.dylib
#   /usr/local/include/zerodds/zerodds.h
#
# Universal Binary:
#   Cargo Target wird zweimal gebaut (aarch64-apple-darwin +
#   x86_64-apple-darwin), dann via `lipo` zu einem fat-Binary
#   zusammengefuehrt.
#
# Code-Signing:
#   Wenn $DEVELOPER_ID gesetzt ist, wird `productsign --sign` genutzt
#   plus `notarytool submit` (Notarisierung erfordert Apple-ID +
#   App-spezifisches PWD ueber Keychain — siehe NOTARYTOOL_KEYCHAIN_PROFILE).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

VERSION="${VERSION:-0.0.0}"
OUT_DIR="${OUT_DIR:-dist/macos}"
TMPROOT="$(mktemp -d -t zerodds-pkg-XXXXXX)"
trap 'rm -rf "$TMPROOT"' EXIT

mkdir -p "$OUT_DIR"

CLI=(dds-admin dds-perf dds-idlc dds-xmlc dds-chaos roundtrip-1us)

build_target () {
    local target="$1"
    rustup target add "$target" >/dev/null
    cargo build --release --target "$target" \
        -p dds-admin -p dds-perf -p dds-idlc -p dds-xmlc \
        -p dds-chaos -p dds-bench-suite -p dds-c-api
}

echo "==> Building aarch64-apple-darwin"
build_target aarch64-apple-darwin
echo "==> Building x86_64-apple-darwin"
build_target x86_64-apple-darwin

# Universal binaries via lipo.
mkdir -p "$TMPROOT/usr/local/bin"
for b in "${CLI[@]}"; do
    lipo -create \
        "target/aarch64-apple-darwin/release/$b" \
        "target/x86_64-apple-darwin/release/$b" \
        -output "$TMPROOT/usr/local/bin/$b"
done

# libzerodds.dylib (universal).
mkdir -p "$TMPROOT/usr/local/lib"
lipo -create \
    "target/aarch64-apple-darwin/release/libzerodds.dylib" \
    "target/x86_64-apple-darwin/release/libzerodds.dylib" \
    -output "$TMPROOT/usr/local/lib/libzerodds.dylib"

# Header.
mkdir -p "$TMPROOT/usr/local/include/zerodds"
cp crates/dds-c-api/include/zerodds.h "$TMPROOT/usr/local/include/zerodds/"

# Component-pkg.
COMPONENT="$OUT_DIR/zerodds-component-$VERSION.pkg"
pkgbuild \
    --root "$TMPROOT" \
    --identifier io.zerodds.zerodds \
    --version "$VERSION" \
    --install-location / \
    "$COMPONENT"

# Distribution-pkg (mit License + Welcome).
DIST_XML="$TMPROOT/distribution.xml"
cat >"$DIST_XML" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>ZeroDDS ${VERSION}</title>
    <organization>io.zerodds</organization>
    <pkg-ref id="io.zerodds.zerodds" version="${VERSION}">zerodds-component-${VERSION}.pkg</pkg-ref>
    <choices-outline>
        <line choice="default">
            <line choice="io.zerodds.zerodds"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="io.zerodds.zerodds" visible="false">
        <pkg-ref id="io.zerodds.zerodds"/>
    </choice>
    <welcome file="welcome.txt"/>
    <license file="../../LICENSE"/>
</installer-gui-script>
EOF

mkdir -p "$TMPROOT/resources"
echo "ZeroDDS — Pure-Rust DDS Toolchain ${VERSION}" >"$TMPROOT/resources/welcome.txt"

OUT_PKG="$OUT_DIR/zerodds-${VERSION}.pkg"
productbuild \
    --distribution "$DIST_XML" \
    --resources "$TMPROOT/resources" \
    --package-path "$OUT_DIR" \
    "$OUT_PKG"

# Code-Signing (optional).
if [[ -n "${DEVELOPER_ID:-}" ]]; then
    echo "==> productsign --sign \"$DEVELOPER_ID\""
    SIGNED="$OUT_DIR/zerodds-${VERSION}-signed.pkg"
    productsign --sign "$DEVELOPER_ID" "$OUT_PKG" "$SIGNED"
    mv "$SIGNED" "$OUT_PKG"

    if [[ -n "${NOTARYTOOL_KEYCHAIN_PROFILE:-}" ]]; then
        echo "==> notarytool submit"
        xcrun notarytool submit "$OUT_PKG" \
            --keychain-profile "$NOTARYTOOL_KEYCHAIN_PROFILE" \
            --wait
        xcrun stapler staple "$OUT_PKG"
    fi
else
    echo "==> Skip signing (DEVELOPER_ID not set)"
fi

echo "==> Done: $OUT_PKG"

#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Spec: docs/specs/zerodds-deployment-1.0.md §2.2.4 —
# AppImage Static-Build pro Daemon.
#
# Baut pro Daemon ein selbsttragendes AppImage (musl-static), benoetigt
# - cargo (mit `x86_64-unknown-linux-musl` Target)
# - appimagetool (https://appimage.github.io/appimagetool/)
#
# Usage:
#   ./build.sh                      # alle 7 Daemons + 17 CLIs
#   ./build.sh zerodds-ws-bridged   # nur einen
#
# Output: ./out/<bin>-<arch>.AppImage

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
OUT_DIR="$SCRIPT_DIR/out"
TARGET="${TARGET:-x86_64-unknown-linux-musl}"
ARCH="${TARGET%%-*}"

DAEMONS=(
    zerodds-ws-bridged
    zerodds-mqtt-bridged
    zerodds-coap-bridged
    zerodds-amqp-bridged
    zerodds-grpc-bridged
    zerodds-corba-bridged
    zerodds-ros2-shim
)

CLIS=(
    zerodds-admin
    zerodds-idlc
    zerodds-spy
    zerodds-record
    zerodds-replay
    zerodds-bench
    zerodds-snitch
    zerodds-monitor
    zerodds-mq
    zerodds-pcap
    zerodds-perf
    zerodds-shape
    zerodds-keys
    zerodds-perm
    zerodds-cert
    zerodds-doctor
    zerodds-license
)

if [[ $# -gt 0 ]]; then
    BINS=("$@")
else
    BINS=("${DAEMONS[@]}" "${CLIS[@]}")
fi

mkdir -p "$OUT_DIR"

ensure_target() {
    rustup target list --installed | grep -q "^${TARGET}$" || \
        rustup target add "$TARGET"
}

build_one() {
    local bin="$1"
    echo "== Building $bin ($TARGET) =="
    (cd "$ROOT_DIR" && cargo build --release --locked --target "$TARGET" --bin "$bin")
}

bundle_one() {
    local bin="$1"
    local appdir="$OUT_DIR/${bin}.AppDir"
    rm -rf "$appdir"
    mkdir -p "$appdir/usr/bin"
    install -Dm755 "$ROOT_DIR/target/$TARGET/release/$bin" "$appdir/usr/bin/$bin"

    # AppRun: deferred-exec wrapper.
    cat > "$appdir/AppRun" << APPRUN
#!/usr/bin/env bash
HERE="\$(dirname "\$(readlink -f "\${0}")")"
exec "\$HERE/usr/bin/${bin}" "\$@"
APPRUN
    chmod +x "$appdir/AppRun"

    # Desktop-Entry (AppImage minimal requirement).
    cat > "$appdir/${bin}.desktop" << DESK
[Desktop Entry]
Name=${bin}
Exec=${bin}
Icon=${bin}
Type=Application
Categories=Utility;Network;
Terminal=true
DESK

    # Stub-Icon (1x1 PNG) — appimagetool requires *some* icon file.
    if [[ ! -f "$appdir/${bin}.png" ]]; then
        printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\rIDATx\x9cc\xf8\xff\xff?\x03\x00\x05\xfe\x02\xfe\xa3X\xc0\x9d\x00\x00\x00\x00IEND\xaeB`\x82' > "$appdir/${bin}.png"
    fi

    if command -v appimagetool >/dev/null 2>&1; then
        ARCH="$ARCH" appimagetool "$appdir" "$OUT_DIR/${bin}-${ARCH}.AppImage"
    else
        echo "  appimagetool not in PATH — skipping pack; AppDir at: $appdir"
    fi
}

ensure_target
for bin in "${BINS[@]}"; do
    build_one "$bin"
    bundle_one "$bin"
done

echo
echo "AppImages in: $OUT_DIR"
ls -la "$OUT_DIR"

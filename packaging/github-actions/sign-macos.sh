#!/usr/bin/env bash
# packaging/github-actions/sign-macos.sh
# Codesign + notarize + .pkg-installer für macOS-binaries.
#
# Spec: docs/specs/zerodds-deployment-1.0.md §3.2 (macOS distribution).
#
# Required env vars (set by release.yml):
#   APPLE_ID                       — Apple-ID email
#   APPLE_TEAM_ID                  — 10-char team identifier
#   APPLE_NOTARY_PASSWORD          — app-specific password for notarytool
#   DEV_ID_APPLICATION             — "Developer ID Application: <name> (TEAMID)"
#   DEV_ID_INSTALLER               — "Developer ID Installer: <name> (TEAMID)"
#                                    (optional — wenn nicht gesetzt, kein .pkg)
#
# Inputs:
#   $1 — directory mit cargo-dist artifacts (typ. target/distrib)
#
# Output (im DIST_DIR):
#   1. Jedes Mach-O-binary signed (hardened-runtime + timestamp)
#   2. Tarballs (.tar.xz) re-packed mit signed binaries + sha512 neu
#   3. Pro target ein zerodds-bundle-<target>.pkg (signed + notarized + stapled)
#   4. Notarize .pkg + .zip (.tar.xz nicht notarisierbar — bleibt
#      gatekeeper-blocked; user-workaround in release-body documentiert)

set -euo pipefail

DIST_DIR="${1:-target/distrib}"
PKG_VERSION="${ZERODDS_PKG_VERSION:-1.0.0-rc.7}"

if [ -z "${DEV_ID_APPLICATION:-}" ]; then
    echo "DEV_ID_APPLICATION not set; skipping macOS signing." >&2
    exit 0
fi
if ! command -v codesign >/dev/null 2>&1; then
    echo "codesign not available; not on macOS?" >&2
    exit 0
fi

# ============================================================
# 1) Codesign every Mach-O binary in DIST_DIR
# ============================================================
echo "==> codesigning binaries in $DIST_DIR"
SIGNED_COUNT=0
while IFS= read -r f; do
    if file "$f" 2>/dev/null | grep -q "Mach-O"; then
        echo "  signing $f"
        codesign --force \
                 --options runtime \
                 --timestamp \
                 --sign "$DEV_ID_APPLICATION" \
                 "$f"
        SIGNED_COUNT=$((SIGNED_COUNT + 1))
    fi
done < <(find "$DIST_DIR" -type f \( -perm -111 -o -name "*.dylib" \))
echo "  signed $SIGNED_COUNT binaries"

# ============================================================
# 2) Re-pack tarballs mit signed binaries
# ============================================================
# cargo-dist hat die tarballs BEVOR signing erstellt; sie enthalten
# adhoc-binaries. Wir re-erstellen jeden tarball aus dem (jetzt
# signed) unzipped-dir.
echo "==> re-packing tarballs"
shopt -s nullglob
for archive in "$DIST_DIR"/*.tar.xz; do
    base="$(basename "$archive" .tar.xz)"
    src_dir="$DIST_DIR/$base"
    if [ -d "$src_dir" ]; then
        echo "  re-pack $base"
        # macOS bsd-tar: --no-mac-metadata strips ._-shadow files
        (cd "$DIST_DIR" && tar --no-mac-metadata -cJf "$base.tar.xz.tmp" "$base")
        mv "$DIST_DIR/$base.tar.xz.tmp" "$archive"
        # sha512 neu berechnen wenn .sha512 file existiert
        if [ -f "$archive.sha512" ]; then
            (cd "$DIST_DIR" && shasum -a 512 "$base.tar.xz" > "$base.tar.xz.sha512")
        fi
    fi
done

# ============================================================
# 3) Build .pkg installer pro target (alle bins zusammen)
# ============================================================
if [ -n "${DEV_ID_INSTALLER:-}" ]; then
    case "$(uname -m)" in
        arm64)  TARGET="aarch64-apple-darwin" ;;
        x86_64) TARGET="x86_64-apple-darwin" ;;
        *)      TARGET="${ZERODDS_TARGET:-unknown-apple-darwin}" ;;
    esac
    PKG_NAME="zerodds-bundle-${PKG_VERSION}-${TARGET}.pkg"
    PKG_OUT="$DIST_DIR/$PKG_NAME"
    PKG_ROOT=$(mktemp -d -t zerodds-pkg.XXXXX)

    echo "==> building .pkg installer: $PKG_NAME"
    mkdir -p "$PKG_ROOT/usr/local/bin"
    # Sammle alle signed binaries aus den unzipped dirs
    BIN_COUNT=0
    for src_dir in "$DIST_DIR"/*-"$TARGET"; do
        if [ -d "$src_dir" ]; then
            for f in "$src_dir"/*; do
                if [ -f "$f" ] && file "$f" 2>/dev/null | grep -q "Mach-O"; then
                    cp "$f" "$PKG_ROOT/usr/local/bin/"
                    BIN_COUNT=$((BIN_COUNT + 1))
                fi
            done
        fi
    done
    echo "  collected $BIN_COUNT binaries into pkg-root"

    if [ "$BIN_COUNT" -gt 0 ]; then
        pkgbuild --root "$PKG_ROOT" \
                 --identifier "org.zerodds.bundle" \
                 --version "$PKG_VERSION" \
                 --install-location "/" \
                 --sign "$DEV_ID_INSTALLER" \
                 --timestamp \
                 "$PKG_OUT"
        echo "  pkgbuild done: $(ls -lh "$PKG_OUT" | awk '{print $5}')"
        # sha512 für .pkg
        (cd "$DIST_DIR" && shasum -a 512 "$PKG_NAME" > "$PKG_NAME.sha512")
    else
        echo "  no binaries collected; skipping pkgbuild" >&2
    fi
    rm -rf "$PKG_ROOT"
else
    echo "==> DEV_ID_INSTALLER not set; skipping .pkg build"
fi

# ============================================================
# 4) Notarize .pkg + .zip (notarytool akzeptiert nur .zip/.pkg/.dmg)
# ============================================================
if [ -z "${APPLE_ID:-}" ] || [ -z "${APPLE_TEAM_ID:-}" ] || [ -z "${APPLE_NOTARY_PASSWORD:-}" ]; then
    echo "==> APPLE_ID/TEAM_ID/NOTARY_PASSWORD not set; skipping notarization"
    exit 0
fi

echo "==> notarizing artifacts"
for archive in "$DIST_DIR"/*.pkg "$DIST_DIR"/*.zip "$DIST_DIR"/*.dmg; do
    [ -f "$archive" ] || continue
    echo "  notarize $(basename "$archive")"
    if xcrun notarytool submit "$archive" \
            --apple-id "$APPLE_ID" \
            --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_NOTARY_PASSWORD" \
            --wait; then
        # Nur .pkg/.dmg/.app sind stapelbar
        case "$archive" in
            *.pkg|*.dmg|*.app)
                xcrun stapler staple "$archive" || echo "  warn: staple failed for $archive"
                ;;
        esac
    else
        echo "  warn: notarize failed for $archive (continuing)"
    fi
done

echo "==> macOS signing + notarization complete"

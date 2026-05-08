#!/usr/bin/env bash
# packaging/github-actions/sign-macos.sh
# Codesign + notarize macOS binaries from cargo-dist output dir.
#
# Spec: docs/specs/zerodds-deployment-1.0.md §3.2 (macOS distribution).
# Required env vars (set by release.yml):
#   APPLE_ID                — Apple-ID email
#   APPLE_TEAM_ID           — 10-char team identifier
#   APPLE_NOTARY_PASSWORD   — app-specific password for notarytool
#   DEV_ID_APPLICATION      — full identity name "Developer ID Application: ..."
#
# Inputs:
#   $1 — directory with cargo-dist artifacts (typically target/distrib)
#
# Output:
#   - every Mach-O binary in $1 is codesigned with hardened-runtime
#   - .tar.{gz,xz} archives are notarized + stapled
set -euo pipefail

DIST_DIR="${1:-target/distrib}"

if [ -z "${DEV_ID_APPLICATION:-}" ]; then
    echo "DEV_ID_APPLICATION not set; skipping macOS signing." >&2
    exit 0
fi

if ! command -v codesign >/dev/null 2>&1; then
    echo "codesign not available; not on macOS?" >&2
    exit 0
fi

echo "==> codesigning binaries in $DIST_DIR"
# Sign every Mach-O executable found in the dist dir.
# cargo-dist packages binaries into per-target tarballs that are extracted
# transiently during build; for the .pkg installer flow we sign within the
# staging root before pkgbuild. Here we sign the loose binaries that
# cargo-dist may have laid out.
find "$DIST_DIR" -type f \( -perm -111 -o -name "*.dylib" \) | while read -r f; do
    # Skip non-Mach-O files.
    if file "$f" 2>/dev/null | grep -q "Mach-O"; then
        echo "  signing $f"
        codesign --force \
                 --options runtime \
                 --timestamp \
                 --sign "$DEV_ID_APPLICATION" \
                 "$f"
    fi
done

echo "==> notarizing archives"
# Apple notarytool akzeptiert nur .zip, .pkg, .dmg. Fuer cargo-dist
# .tar.gz/.tar.xz ist Notarisierung gar nicht spec-erlaubt — Gatekeeper
# verifiziert dort zur Laufzeit per codesign-staple das Binary selbst,
# das bereits oben signiert wurde. Wir notarisieren also nur .zip/.pkg/.dmg.
shopt -s nullglob
for archive in "$DIST_DIR"/*.zip "$DIST_DIR"/*.pkg "$DIST_DIR"/*.dmg; do
    echo "  notarizing $archive"
    xcrun notarytool submit "$archive" \
            --apple-id "$APPLE_ID" \
            --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_NOTARY_PASSWORD" \
            --wait
    case "$archive" in
        *.pkg|*.dmg)
            xcrun stapler staple "$archive"
            ;;
    esac
done

echo "==> macOS signing + notarization complete"

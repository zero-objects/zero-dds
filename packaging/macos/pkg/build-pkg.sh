#!/usr/bin/env bash
# packaging/macos/pkg/build-pkg.sh
# Builds a notarized .pkg installer for ZeroDDS on macOS.
# Spec: zerodds-deployment-1.0.md §3.2.2.
set -euo pipefail

VERSION="${1:-1.0.0-rc.7}"
ARCH="${2:-universal2}"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

ROOT="$WORK/root"
SCRIPTS="$WORK/scripts"
mkdir -p "$ROOT/usr/local/bin" \
         "$ROOT/usr/local/lib" \
         "$ROOT/usr/local/include" \
         "$ROOT/usr/local/etc/zerodds" \
         "$ROOT/usr/local/var/log/zerodds" \
         "$ROOT/usr/local/var/lib/zerodds" \
         "$ROOT/usr/local/share/man/man1" \
         "$ROOT/usr/local/share/man/man5" \
         "$ROOT/Library/LaunchDaemons" \
         "$SCRIPTS"

# Stage payload.
cp target/release/zerodds-* "$ROOT/usr/local/bin/"
cp target/release/libzerodds.dylib "$ROOT/usr/local/lib/"
cp crates/zerodds-c-api/include/zerodds.h "$ROOT/usr/local/include/"
cp packaging/linux/configs/*.yaml.example "$ROOT/usr/local/etc/zerodds/"
cp packaging/macos/launchd/*.plist "$ROOT/Library/LaunchDaemons/"
cp man/man1/zerodds-*.1 "$ROOT/usr/local/share/man/man1/"
cp man/man5/zerodds-*.yaml.5 "$ROOT/usr/local/share/man/man5/"

# Pre-/postinstall: System-User _zerodds anlegen, Service registrieren.
cat > "$SCRIPTS/preinstall" <<'PRE'
#!/bin/sh
set -e
if ! dscl . -read /Users/_zerodds >/dev/null 2>&1; then
    NEXT_UID=$(dscl . -list /Users UniqueID | awk '$2 < 500 { print $2 }' \
               | sort -n | tail -1 | awk '{print $1+1}')
    dscl . -create /Users/_zerodds
    dscl . -create /Users/_zerodds UniqueID "$NEXT_UID"
    dscl . -create /Users/_zerodds PrimaryGroupID 1
    dscl . -create /Users/_zerodds NFSHomeDirectory /var/empty
    dscl . -create /Users/_zerodds UserShell /usr/bin/false
    dscl . -create /Users/_zerodds RealName "ZeroDDS Service"
fi
exit 0
PRE
chmod +x "$SCRIPTS/preinstall"

cat > "$SCRIPTS/postinstall" <<'POST'
#!/bin/sh
set -e
chown -R _zerodds:wheel /usr/local/var/log/zerodds /usr/local/var/lib/zerodds /usr/local/etc/zerodds
launchctl load -w /Library/LaunchDaemons/org.zerodds.ws-bridged.plist || true
exit 0
POST
chmod +x "$SCRIPTS/postinstall"

PKG="zerodds-${VERSION}-${ARCH}.pkg"
pkgbuild --root "$ROOT" \
         --scripts "$SCRIPTS" \
         --identifier org.zerodds.installer \
         --version "$VERSION" \
         --install-location "/" \
         "dist/$PKG"

# Optional: notarytool submit + staple.
if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    productsign --sign "Developer ID Installer: ZeroDDS" \
                "dist/$PKG" "dist/${PKG%.pkg}-signed.pkg"
    xcrun notarytool submit "dist/${PKG%.pkg}-signed.pkg" \
                --apple-id "$APPLE_ID" \
                --team-id "$APPLE_TEAM_ID" \
                --password "$APPLE_NOTARY_PASSWORD" \
                --wait
    xcrun stapler staple "dist/${PKG%.pkg}-signed.pkg"
fi

echo "Built $PKG"

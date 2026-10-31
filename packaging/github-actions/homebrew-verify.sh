#!/usr/bin/env bash
# packaging/github-actions/homebrew-verify.sh
# Installs rendered Homebrew formulae on a macOS runner before
# publish-distros.yml pushes them to zero-objects/homebrew-zerodds: the meta
# formula with its `brew test`, then every sub-formula. Homebrew installs
# formulae only from a tap, so the files go into a throwaway local tap.
# The runner's CPU decides which of the two bundles is installed.
# Spec: zerodds-deployment-1.0.md §3.2.1.
#
# Usage: homebrew-verify.sh <directory with the rendered *.rb>
set -euo pipefail
FORMULA_DIR="${1:?directory with the rendered formulae required}"
[ -f "$FORMULA_DIR/zerodds.rb" ] || { echo "no zerodds.rb in $FORMULA_DIR" >&2; exit 1; }
if grep -q 'sha256 "0\{64\}"' "$FORMULA_DIR"/*.rb; then
    echo "a formula still carries the placeholder sha256" >&2
    exit 1
fi

TAP="zerodds-ci/verify"
brew tap-new --no-git "$TAP" >/dev/null
trap 'brew untap --force "$TAP" >/dev/null 2>&1 || true' EXIT
cp "$FORMULA_DIR"/*.rb "$(brew --repository "$TAP")/Formula/"

brew install --formula "$TAP/zerodds"
brew test "$TAP/zerodds"
prefix="$(brew --prefix)"
"$prefix/bin/zerodds-admin" --version
test -f "$prefix/lib/libzerodds.dylib"
test -f "$prefix/include/zerodds.h"
PKG_CONFIG_PATH="$prefix/lib/pkgconfig" pkg-config --modversion zerodds
test -f "$prefix/etc/zerodds/ws-bridged.yaml"

for f in "$FORMULA_DIR"/zerodds-*.rb; do
    brew install --formula "$TAP/$(basename "$f" .rb)"
done
# The ROS-2 sub-formula is the only one whose configuration the meta formula
# does not already place.
test -f "$prefix/etc/zerodds/ros2.yaml"
brew list --formula | grep '^zerodds'

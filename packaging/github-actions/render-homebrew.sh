#!/usr/bin/env bash
# packaging/github-actions/render-homebrew.sh
# Substitutes the artefact SHA-256 placeholders in the Homebrew formulae and
# copies them into the homebrew-tap checkout. Called by release.yml ->
# publish-homebrew-tap. Spec: zerodds-deployment-1.0.md §3.2.1.
set -euo pipefail
TAP_DIR="${1:?tap-dir required}"
mkdir -p "$TAP_DIR"

DIST="${DIST_DIR:-target/distrib}"
sha () { sha256sum "$1" | awk '{print $1}'; }

ARM_TARBALL="$DIST/zerodds-aarch64-apple-darwin.tar.gz"
X64_TARBALL="$DIST/zerodds-x86_64-apple-darwin.tar.gz"

ARM_SHA=$([ -f "$ARM_TARBALL" ] && sha "$ARM_TARBALL" || echo "0000000000000000000000000000000000000000000000000000000000000000")
X64_SHA=$([ -f "$X64_TARBALL" ] && sha "$X64_TARBALL" || echo "0000000000000000000000000000000000000000000000000000000000000000")

for f in packaging/macos/homebrew/*.rb; do
    out="$TAP_DIR/$(basename "$f")"
    sed -e "0,/0000000000000000000000000000000000000000000000000000000000000000/{s//$ARM_SHA/}" \
        -e "s/0000000000000000000000000000000000000000000000000000000000000000/$X64_SHA/" \
        "$f" > "$out"
done

echo "Rendered $(ls "$TAP_DIR" | wc -l) formulae into $TAP_DIR."

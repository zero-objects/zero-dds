#!/usr/bin/env bash
# packaging/github-actions/sign-artifacts.sh
# Signs every artefact under $1 with minisign and cosign.
# Spec: zerodds-deployment-1.0.md §6 (sign-artifacts = ["minisign", "cosign"]).
set -euo pipefail
DIST_DIR="${1:-target/distrib}"

if [ -z "${MINISIGN_KEY:-}" ]; then
    echo "MINISIGN_KEY not provided — skipping minisign step." >&2
else
    echo "$MINISIGN_KEY" > /tmp/minisign.key
    for f in "$DIST_DIR"/*; do
        [ -f "$f" ] || continue
        echo "$MINISIGN_PWD" | minisign -Sm "$f" -s /tmp/minisign.key
    done
    rm /tmp/minisign.key
fi

if command -v cosign >/dev/null 2>&1; then
    for f in "$DIST_DIR"/*; do
        [ -f "$f" ] || continue
        # OIDC keyless via GH-Actions identity token.
        cosign sign-blob --yes "$f" --output-signature "$f.sig" \
                                    --output-certificate "$f.pem"
    done
fi

# SHA-512 manifest.
( cd "$DIST_DIR" && sha512sum -- * > SHA512SUMS )
echo "Signed $(ls "$DIST_DIR" | wc -l) artefacts."

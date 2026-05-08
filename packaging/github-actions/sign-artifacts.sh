#!/usr/bin/env bash
# packaging/github-actions/sign-artifacts.sh
# Signs every artefact under $1 with minisign and cosign.
# Spec: zerodds-deployment-1.0.md §6 (sign-artifacts = ["minisign", "cosign"]).
set -euo pipefail
DIST_DIR="${1:-target/distrib}"

if [ -z "${MINISIGN_KEY:-}" ]; then
    echo "MINISIGN_KEY not provided — skipping minisign step." >&2
else
    # Auto-install minisign auf Ubuntu runners. Ab 24.04 in apt; auf
    # 22.04 (cargo-dist default) nicht im offiziellen archive →
    # prebuilt binary von github releases ziehen.
    if ! command -v minisign >/dev/null 2>&1; then
        if sudo apt-get install -y -qq minisign 2>/dev/null; then
            echo "installed minisign via apt" >&2
        else
            echo "installing minisign from upstream binary…" >&2
            MS_VER="0.12"
            curl -fsSL "https://github.com/jedisct1/minisign/releases/download/${MS_VER}/minisign-${MS_VER}-linux.tar.gz" -o /tmp/minisign.tar.gz
            sudo tar -xzf /tmp/minisign.tar.gz -C /tmp
            sudo install -m755 "/tmp/minisign-linux/x86_64/minisign" /usr/local/bin/minisign
            # Cleanup mit sudo weil tar-extract als root extrahiert hat.
            sudo rm -rf /tmp/minisign.tar.gz /tmp/minisign-linux
        fi
    fi
    echo "$MINISIGN_KEY" > /tmp/minisign.key
    for f in "$DIST_DIR"/*; do
        [ -f "$f" ] || continue
        # Skip non-regular files (already-signed .minisig files etc).
        case "$f" in *.minisig) continue;; esac
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

# SHA-512 manifest. Nur regulaere Files; cargo-dist legt parallel
# zu den .tar.{gz,xz}-Archiven auch ihre extrahierten Verzeichnisse
# in `distrib/` ab — `sha512sum` auf einem Dir failt mit "Is a directory".
(
    cd "$DIST_DIR"
    # shellcheck disable=SC2035
    find . -maxdepth 1 -type f ! -name "SHA512SUMS" -printf "%f\n" \
        | sort \
        | xargs -r sha512sum > SHA512SUMS
)
echo "Signed + hashed $(find "$DIST_DIR" -maxdepth 1 -type f | wc -l) artefacts."

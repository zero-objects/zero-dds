#!/usr/bin/env bash
# packaging/github-actions/sign-artifacts.sh
# Signs the given files with minisign (FILE.minisig) and, with --cosign,
# keyless with cosign (FILE.cosign.bundle). release.yml calls it once, for
# the SHA512SUMS over all release files, so one signature of each kind covers
# the whole release.
# Spec: zerodds-deployment-1.0.md §6 (sign-artifacts = ["minisign", "cosign"]).
#
# Usage:
#   MINISIGN_KEY=... MINISIGN_PWD=... \
#   sign-artifacts.sh [--pubkey minisign.pub] [--comment TEXT] [--cosign] FILE...
#
# --pubkey  verifies every minisign signature against this public key right
#           after signing, so a secret key that does not belong to the
#           published public key fails the release instead of shipping.
# --cosign  needs `cosign` on PATH and an OIDC identity (GitHub Actions with
#           `id-token: write`); the bundle holds signature and certificate.
#
# A missing key or tool is an error: a release without its signatures is not
# a release.
set -euo pipefail

PUBKEY=""
COMMENT=""
COSIGN=0
FILES=()
while [ $# -gt 0 ]; do
    case "$1" in
        --pubkey)  PUBKEY="$2";  shift 2;;
        --comment) COMMENT="$2"; shift 2;;
        --cosign)  COSIGN=1;     shift;;
        --) shift; FILES+=("$@"); break;;
        -*) echo "unknown option: $1" >&2; exit 2;;
        *) FILES+=("$1"); shift;;
    esac
done
[ "${#FILES[@]}" -gt 0 ] || { echo "no files to sign" >&2; exit 2; }
for f in "${FILES[@]}"; do
    [ -f "$f" ] && [ -s "$f" ] || { echo "not a nonempty file: $f" >&2; exit 1; }
done
[ -n "${MINISIGN_KEY:-}" ] || { echo "MINISIGN_KEY is not set" >&2; exit 1; }

# minisign is in apt from Ubuntu 24.04 on; on 22.04 take the upstream binary.
if ! command -v minisign >/dev/null 2>&1; then
    if sudo apt-get install -y -qq minisign 2>/dev/null; then
        echo "installed minisign via apt" >&2
    else
        echo "installing minisign from the upstream binary" >&2
        MS_VER="0.12"
        tmp="$(mktemp -d)"
        curl -fsSL "https://github.com/jedisct1/minisign/releases/download/${MS_VER}/minisign-${MS_VER}-linux.tar.gz" -o "$tmp/minisign.tar.gz"
        tar -xzf "$tmp/minisign.tar.gz" -C "$tmp"
        sudo install -m755 "$tmp/minisign-linux/x86_64/minisign" /usr/local/bin/minisign
        rm -rf "$tmp"
    fi
fi

keyfile="$(mktemp)"
trap 'rm -f "$keyfile"' EXIT
printf '%s\n' "$MINISIGN_KEY" > "$keyfile"
for f in "${FILES[@]}"; do
    args=(-S -s "$keyfile" -m "$f")
    [ -n "$COMMENT" ] && args+=(-t "$COMMENT")
    printf '%s\n' "${MINISIGN_PWD:-}" | minisign "${args[@]}"
    if [ -n "$PUBKEY" ]; then
        minisign -V -p "$PUBKEY" -m "$f" >/dev/null
    fi
    echo "minisign: $f.minisig"
done

if [ "$COSIGN" = 1 ]; then
    command -v cosign >/dev/null 2>&1 || { echo "--cosign given, cosign not on PATH" >&2; exit 1; }
    for f in "${FILES[@]}"; do
        cosign sign-blob --yes --bundle "$f.cosign.bundle" "$f" >/dev/null
        echo "cosign: $f.cosign.bundle"
    done
fi

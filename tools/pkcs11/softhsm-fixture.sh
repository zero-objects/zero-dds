#!/usr/bin/env bash
# softhsm-fixture.sh — a SoftHSM token with a P-256 identity, for the
# `pkcs11:` integration test (DDS-Security 1.2 §9.3.1 Table 44, RFC 7512).
#
# Creates a throwaway SoftHSM token, imports a P-256 certificate signed by
# a test CA, the CA certificate and the matching private key, and prints
# the `pkcs11:` URIs that name them. The private key is imported into the
# token; it is never handed to the test as a file, which is the point of
# the `pkcs11:` scheme.
#
# Only the URIs (and the two `KEY=VALUE` lines the test needs) go to
# stdout, one per line; every command's chatter goes to stderr. The test
# parses stdout.
#
# Tools: `softhsm2-util` and `pkcs11-tool` (OpenSC) and `openssl`. The
# module defaults to the Linux test host's SoftHSM 2.6.1 path and can be
# overridden with PKCS11_MODULE. The token store lives under a fresh
# mktemp directory and `SOFTHSM2_CONF` points at it, so the host's own
# SoftHSM configuration is untouched.
set -euo pipefail

MODULE="${PKCS11_MODULE:-/usr/lib/softhsm/libsofthsm2.so}"
TOKEN_LABEL="ZeroDDS"
PIN="1234"
SO_PIN="0000"

if [ ! -e "$MODULE" ]; then
    echo "softhsm-fixture: PKCS11_MODULE '$MODULE' does not exist" >&2
    exit 1
fi
for tool in softhsm2-util pkcs11-tool openssl; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "softhsm-fixture: '$tool' is not installed" >&2
        exit 1
    fi
done

WORK="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-pkcs11.XXXXXX")"
export SOFTHSM2_CONF="$WORK/softhsm2.conf"
mkdir -p "$WORK/tokens"
cat >"$SOFTHSM2_CONF" <<EOF
directories.tokendir = $WORK/tokens
objectstore.backend = file
log.level = ERROR
EOF

echo "softhsm-fixture: token store $WORK/tokens" >&2
softhsm2-util --init-token --free --label "$TOKEN_LABEL" --so-pin "$SO_PIN" --pin "$PIN" >&2

# Test CA (P-256), self-signed.
openssl ecparam -name prime256v1 -genkey -noout -out "$WORK/ca.key" >&2 2>&1
openssl req -x509 -new -key "$WORK/ca.key" -out "$WORK/ca.pem" -days 3650 \
    -subj "/CN=ZeroDDS Test CA" >&2 2>&1

# Identity key (P-256) and a certificate signed by the CA.
openssl ecparam -name prime256v1 -genkey -noout -out "$WORK/identity.key" >&2 2>&1
openssl req -new -key "$WORK/identity.key" -out "$WORK/identity.csr" \
    -subj "/CN=zerodds-identity" >&2 2>&1
openssl x509 -req -in "$WORK/identity.csr" -CA "$WORK/ca.pem" -CAkey "$WORK/ca.key" \
    -CAcreateserial -out "$WORK/identity.pem" -days 3650 >&2 2>&1
openssl pkcs8 -topk8 -nocrypt -in "$WORK/identity.key" -outform DER \
    -out "$WORK/identity.p8" >&2 2>&1

# CA certificate (id 0x02) and identity certificate (id 0x01).
pkcs11-tool --module "$MODULE" --token-label "$TOKEN_LABEL" --login --pin "$PIN" \
    --write-object "$WORK/ca.pem" --type cert --label identity_ca --id 02 >&2
pkcs11-tool --module "$MODULE" --token-label "$TOKEN_LABEL" --login --pin "$PIN" \
    --write-object "$WORK/identity.pem" --type cert --label identity --id 01 >&2
# Private key (id 0x01), PKCS#8 DER.
pkcs11-tool --module "$MODULE" --token-label "$TOKEN_LABEL" --login --pin "$PIN" \
    --write-object "$WORK/identity.p8" --type privkey --label identity --id 01 >&2

printf 'pkcs11:token=%s;object=identity_ca;id=%%02;type=cert?module-path=%s\n' \
    "$TOKEN_LABEL" "$MODULE"
printf 'pkcs11:token=%s;object=identity;id=%%01;type=cert?module-path=%s\n' \
    "$TOKEN_LABEL" "$MODULE"
# A private object is visible only after C_Login, so its URI carries the PIN
# (RFC 7512 §2.4 pin-value).
printf 'pkcs11:token=%s;object=identity;id=%%01;type=private?module-path=%s&pin-value=%s\n' \
    "$TOKEN_LABEL" "$MODULE" "$PIN"
printf 'SOFTHSM2_CONF=%s\n' "$SOFTHSM2_CONF"
printf 'CA_PEM=%s\n' "$WORK/ca.pem"

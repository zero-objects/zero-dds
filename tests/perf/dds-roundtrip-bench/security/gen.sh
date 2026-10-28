#!/usr/bin/env bash
# Regenerates the test CA + identity certs + permissions/governance .p7s
# for the DDS-Security cross-vendor bench.
#
# **FOR BENCH/TEST ONLY**: the `identity_ca` is self-signed with a hardcoded
# subject; the private keys are unencrypted. NOT for production.
#
# Reproducible: each bench operator generates their own test CA
# locally — we do NOT track the generated keys/certs/p7s in git
# (see .gitignore in this dir).
#
# Usage:
#   bash gen.sh              # EC certs (Cyclone/OpenDDS/ZeroDDS) — default
#   KEY_ALGO=RSA bash gen.sh # RSA certs (needed for FastDDS-secured cross-vendor)

set -e
cd "$(dirname "$0")"
mkdir -p certs

# Key algorithm of the identity certs: EC P-256 (default) or RSA-2048.
#   KEY_ALGO=EC  (default): accepted by Cyclone, OpenDDS and ZeroDDS.
#   KEY_ALGO=RSA          : needed for FastDDS-secured CROSS-VENDOR. FastDDS'
#     PKI-DH handshake rejects EC identity certs cross-vendor — FastDDS<->
#     {Cyclone,OpenDDS,ZeroDDS} secured fails with EC ("match timeout"),
#     works with RSA. This is on the VENDOR side (FastDDS), reproducible
#     without ZeroDDS (cyclone<->fastdds-EC fails as well); FastDDS<->FastDDS
#     works with EC. ZeroDDS interoperates secured with both algorithms.
#   `genpkey` delivers PKCS#8 (`BEGIN PRIVATE KEY`); ZeroDDS' PKI plugin
#   (rustls-pki-types) expects PKCS#8 strictly.
KEY_ALGO="${KEY_ALGO:-EC}"
genkey() {  # $1 = output key path
    case "$KEY_ALGO" in
        RSA) openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$1" ;;
        *)   openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$1" ;;
    esac
}

# 1) Identity CA (root) — DDS-Security 1.2 §9.3.1.
genkey certs/identity_ca_key.pem
openssl req -x509 -new -nodes -key certs/identity_ca_key.pem -days 365 \
    -subj "/CN=ZeroDDS Bench Identity CA" -out certs/identity_ca.pem

# 2) Permissions CA — for tests = identity CA.
cp certs/identity_ca_key.pem certs/permissions_ca_key.pem
cp certs/identity_ca.pem      certs/permissions_ca.pem

# 3) Per-participant identity certs (ping + pong), signed by the identity CA.
for who in ping pong; do
    genkey certs/${who}_key.pem
    openssl req -new -key certs/${who}_key.pem \
        -subj "/CN=zerodds-bench-${who}/emailAddress=${who}@bench.zerodds.local" \
        -out /tmp/${who}.csr
    openssl x509 -req -in /tmp/${who}.csr \
        -CA certs/identity_ca.pem -CAkey certs/identity_ca_key.pem \
        -CAcreateserial -days 365 -out certs/${who}_cert.pem
    rm /tmp/${who}.csr
done

# 4) Governance + permissions via CMS (RFC 5652 SignedData) signed with the
#    **Permissions CA cert ITSELF** — this is the documented DDS-Security
#    convention of all vendors:
#      FastDDS docs (Access Permissions): `openssl smime -sign -signer
#        maincacert.pem -inkey maincakey.pem` — the signer IS the CA.
#      ZettaScale/Cyclone KB + OpenDDS guide: identical.
#    FastDDS' PKCS7 verify uses the `smimesign` purpose (EKU emailProtection);
#    a separate EE signer cert WITHOUT this EKU fails ("PKCS7 data
#    verification failed", Permissions.cpp). ZeroDDS' CmsPkcs7Verifier
#    accepts the CA self-signed pattern (self-trust). => ONE asset tree
#    that cyclone/fastdds/opendds/zerodds all verify.
openssl smime -sign -in governance.xml -text -out governance.p7s \
    -signer certs/permissions_ca.pem -inkey certs/permissions_ca_key.pem
for who in ping pong; do
    openssl smime -sign -in permissions_${who}.xml -text -out permissions_${who}.p7s \
        -signer certs/permissions_ca.pem -inkey certs/permissions_ca_key.pem
done

# 5) Cyclone security XML config generator.
#    CYCLONEDDS_URI needs absolute `file://` paths to the assets. Instead of
#    hardcoding them, we generate the XMLs here with the absolute path resolved
#    at generation time ($PWD = this security/ dir) — so the
#    matrix runs from ANY checkout, without external hardcoded paths or manual
#    editing. fastdds/opendds/zerodds, by contrast, read their paths generically
#    from ZERODDS_BENCH_SEC_DIR (programmatically in the *_app.cpp), and so
#    need no generated file.
ABS="$(pwd)"
for who in ping pong; do
    cat > "cyclone_security_${who}.xml" <<XML
<CycloneDDS xmlns="https://cdds.io/config">
  <Domain>
    <Security>
      <Authentication>
        <Library finalizeFunction="finalize_authentication" initFunction="init_authentication" path="dds_security_auth"/>
        <IdentityCertificate>file://${ABS}/certs/${who}_cert.pem</IdentityCertificate>
        <IdentityCA>file://${ABS}/certs/identity_ca.pem</IdentityCA>
        <PrivateKey>file://${ABS}/certs/${who}_key.pem</PrivateKey>
      </Authentication>
      <AccessControl>
        <Library finalizeFunction="finalize_access_control" initFunction="init_access_control" path="dds_security_ac"/>
        <PermissionsCA>file://${ABS}/certs/permissions_ca.pem</PermissionsCA>
        <Governance>file://${ABS}/governance.p7s</Governance>
        <Permissions>file://${ABS}/permissions_${who}.p7s</Permissions>
      </AccessControl>
      <Cryptographic>
        <Library finalizeFunction="finalize_crypto" initFunction="init_crypto" path="dds_security_crypto"/>
      </Cryptographic>
    </Security>
  </Domain>
</CycloneDDS>
XML
done

echo "OK — generated:"
ls -1 certs/ *.p7s cyclone_security_*.xml

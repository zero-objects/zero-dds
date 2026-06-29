#!/usr/bin/env bash
# Regeneriert das Test-CA + Identity-Certs + Permissions/Governance.p7s
# für DDS-Security-Cross-Vendor-Bench.
#
# **NUR FÜR BENCH/TEST**: das `identity_ca` ist self-signed mit hardcoded
# Subject; die Private-Keys sind unverschlüsselt. NICHT für Production.
#
# Reproduzierbar: jeder Bench-Operator generiert sich lokal sein eigenes
# Test-CA — wir tracken NICHT die generierten Keys/Certs/p7s im git
# (siehe .gitignore in diesem Dir).
#
# Aufruf:
#   bash gen.sh              # EC-Certs (Cyclone/OpenDDS/ZeroDDS) — Default
#   KEY_ALGO=RSA bash gen.sh # RSA-Certs (noetig fuer FastDDS-secured cross-vendor)

set -e
cd "$(dirname "$0")"
mkdir -p certs

# Schluesselalgorithmus der Identity-Certs: EC P-256 (Default) oder RSA-2048.
#   KEY_ALGO=EC  (Default): von Cyclone, OpenDDS und ZeroDDS akzeptiert.
#   KEY_ALGO=RSA          : noetig fuer FastDDS-secured CROSS-VENDOR. FastDDS'
#     PKI-DH-Handshake lehnt EC-Identity-Certs cross-vendor ab — FastDDS<->
#     {Cyclone,OpenDDS,ZeroDDS} secured scheitert mit EC ("match timeout"),
#     funktioniert mit RSA. Das ist VENDOR-seitig (FastDDS), ohne ZeroDDS
#     reproduzierbar (cyclone<->fastdds-EC scheitert ebenso); FastDDS<->FastDDS
#     geht mit EC. ZeroDDS interoperiert secured mit beiden Algorithmen.
#   `genpkey` liefert PKCS#8 (`BEGIN PRIVATE KEY`); ZeroDDS' PKI-Plugin
#   (rustls-pki-types) erwartet PKCS#8 strikt.
KEY_ALGO="${KEY_ALGO:-EC}"
genkey() {  # $1 = output key path
    case "$KEY_ALGO" in
        RSA) openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$1" ;;
        *)   openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$1" ;;
    esac
}

# 1) Identity-CA (Root) — DDS-Security 1.2 §9.3.1.
genkey certs/identity_ca_key.pem
openssl req -x509 -new -nodes -key certs/identity_ca_key.pem -days 365 \
    -subj "/CN=ZeroDDS Bench Identity CA" -out certs/identity_ca.pem

# 2) Permissions-CA — für Tests = Identity-CA.
cp certs/identity_ca_key.pem certs/permissions_ca_key.pem
cp certs/identity_ca.pem      certs/permissions_ca.pem

# 3) Per-Participant Identity-Certs (ping + pong), signiert von der Identity-CA.
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

# 4) Governance + Permissions via CMS (RFC 5652 SignedData) signiert mit dem
#    **Permissions-CA-Cert SELBST** — das ist die dokumentierte DDS-Security-
#    Konvention aller Vendoren:
#      FastDDS-Docs (Access-Permissions): `openssl smime -sign -signer
#        maincacert.pem -inkey maincakey.pem` — der Signer IST die CA.
#      ZettaScale/Cyclone-KB + OpenDDS-Guide: identisch.
#    FastDDS' PKCS7-Verify nutzt die `smimesign`-Purpose (EKU emailProtection);
#    ein separates EE-Signer-Cert OHNE diese EKU faellt durch ("PKCS7 data
#    verification failed", Permissions.cpp). ZeroDDS' CmsPkcs7Verifier
#    akzeptiert das CA-self-signed-Pattern (self-trust). => EIN Asset-Tree,
#    den cyclone/fastdds/opendds/zerodds alle verifizieren.
openssl smime -sign -in governance.xml -text -out governance.p7s \
    -signer certs/permissions_ca.pem -inkey certs/permissions_ca_key.pem
for who in ping pong; do
    openssl smime -sign -in permissions_${who}.xml -text -out permissions_${who}.p7s \
        -signer certs/permissions_ca.pem -inkey certs/permissions_ca_key.pem
done

# 5) Cyclone-Security-XML-Config-Generator.
#    CYCLONEDDS_URI braucht absolute `file://`-Pfade auf die Assets. Statt
#    sie hart zu kodieren, generieren wir die XMLs hier mit dem zur Gen-Zeit
#    aufgeloesten Absolutpfad ($PWD = dieses security/-Dir) — so laeuft die
#    Matrix aus JEDEM Checkout, ohne externe Hardcode-Pfade oder manuelles
#    Editieren. fastdds/opendds/zerodds lesen ihre Pfade dagegen generisch
#    aus ZERODDS_BENCH_SEC_DIR (programmatisch in den *_app.cpp), brauchen
#    also keine generierte Datei.
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

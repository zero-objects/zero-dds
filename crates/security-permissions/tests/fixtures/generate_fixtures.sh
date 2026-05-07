#!/usr/bin/env bash
# Generates S/MIME PKCS#7-Detached signed Permissions XML fixtures for
# the CmsPkcs7Verifier integration tests.
#
# Run from this directory:
#     cd crates/security-permissions/tests/fixtures
#     ./generate_fixtures.sh
#
# Produces (committed to repo):
#   - permissions_ca_cert.pem            (Permissions-CA — Trust-Anchor)
#   - wrong_ca_cert.pem                  (rogue CA used by negative test)
#   - permissions.xml                    (cleartext permissions doc)
#   - valid_permissions.smime.p7s        (opaque-signed: smime-type=signed-data)
#   - valid_permissions_multipart.smime.p7s (multipart/signed detached)
#   - valid_permissions_pem.p7s          (PEM-wrapped detached PKCS#7)
#   - valid_permissions_xml.txt          (matching XML for PEM variant)
#   - wrong_ca_permissions.smime.p7s     (different-CA signature → reject)
#   - tampered_permissions.smime.p7s     (XML mutated post-sign → reject)
#   - intermediate_chain.smime.p7s       (signer via intermediate CA)
#   - expired_signer.smime.p7s           (EE not_after in past → reject)
#   - future_signer.smime.p7s            (EE not_before in future → reject)
#   - rsa_pkcs1_signer.smime.p7s         (RSA-PKCS#1-v1.5 signer)

set -euo pipefail
cd "$(dirname "$0")"

# Cleanup
rm -f *.pem *.p7s *.txt *.xml *.cnf *.srl 2>/dev/null
rm -rf ca_db expired_ca_db future_ca_db 2>/dev/null

# -------------------- ECDSA P-256 root CA (Permissions-CA) --------------------
openssl ecparam -genkey -name prime256v1 -noout -out permissions_ca_key.pem
openssl req -x509 -new -nodes -key permissions_ca_key.pem -sha256 -days 3650 \
    -subj "/CN=ZeroDDS Permissions CA/O=ZeroDDS/C=DE" \
    -out permissions_ca_cert.pem

# -------------------- ECDSA P-256 signer EE (direct child) --------------------
openssl ecparam -genkey -name prime256v1 -noout -out signer_ec_key.pem
openssl req -new -key signer_ec_key.pem \
    -subj "/CN=permissions-signer/O=ZeroDDS/C=DE" \
    -out signer_ec.csr
openssl x509 -req -in signer_ec.csr -CA permissions_ca_cert.pem \
    -CAkey permissions_ca_key.pem -CAcreateserial \
    -days 3650 -sha256 -out signer_ec_cert.pem

# -------------------- Wrong-CA root + signer --------------------
openssl ecparam -genkey -name prime256v1 -noout -out wrong_ca_key.pem
openssl req -x509 -new -nodes -key wrong_ca_key.pem -sha256 -days 3650 \
    -subj "/CN=Rogue CA/O=Rogue/C=DE" -out wrong_ca_cert.pem
openssl ecparam -genkey -name prime256v1 -noout -out wrong_signer_key.pem
openssl req -new -key wrong_signer_key.pem \
    -subj "/CN=rogue-signer/O=Rogue/C=DE" -out wrong_signer.csr
openssl x509 -req -in wrong_signer.csr -CA wrong_ca_cert.pem \
    -CAkey wrong_ca_key.pem -CAcreateserial \
    -days 3650 -sha256 -out wrong_signer_cert.pem

# -------------------- Intermediate CA + signer --------------------
cat > intermediate_v3.cnf <<EOF
[v3_ca]
basicConstraints = critical, CA:TRUE, pathlen:0
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
EOF
openssl ecparam -genkey -name prime256v1 -noout -out intermediate_ca_key.pem
openssl req -new -key intermediate_ca_key.pem \
    -subj "/CN=ZeroDDS Intermediate CA/O=ZeroDDS/C=DE" -out intermediate.csr
openssl x509 -req -in intermediate.csr \
    -CA permissions_ca_cert.pem -CAkey permissions_ca_key.pem \
    -CAcreateserial -days 3650 -sha256 \
    -extfile intermediate_v3.cnf -extensions v3_ca \
    -out intermediate_ca_cert.pem
openssl ecparam -genkey -name prime256v1 -noout -out int_signer_key.pem
openssl req -new -key int_signer_key.pem \
    -subj "/CN=int-signer/O=ZeroDDS/C=DE" -out int_signer.csr
openssl x509 -req -in int_signer.csr -CA intermediate_ca_cert.pem \
    -CAkey intermediate_ca_key.pem -CAcreateserial \
    -days 3650 -sha256 -out int_signer_cert.pem

# -------------------- Expired signer --------------------
# Use `openssl ca` with an explicit -enddate in the past.
mkdir -p expired_ca_db
touch expired_ca_db/index.txt
echo 1000 > expired_ca_db/serial
cat > expired_ca_setup.cnf <<EOF
[ca]
default_ca = expired_section
[expired_section]
database = expired_ca_db/index.txt
serial = expired_ca_db/serial
new_certs_dir = expired_ca_db
private_key = permissions_ca_key.pem
certificate = permissions_ca_cert.pem
default_md = sha256
default_days = 1
default_crl_days = 30
policy = pol
[pol]
commonName = supplied
organizationName = optional
countryName = optional
EOF
openssl ecparam -genkey -name prime256v1 -noout -out expired_signer_key.pem
openssl req -new -key expired_signer_key.pem \
    -subj "/CN=expired-signer/O=ZeroDDS/C=DE" -out expired_signer.csr
# Explicit historical end date (note: -enddate format YYMMDDHHMMSSZ).
openssl ca -batch -config expired_ca_setup.cnf \
    -in expired_signer.csr -out expired_signer_cert.pem \
    -enddate 200101010000Z -notext

# -------------------- Future signer --------------------
mkdir -p future_ca_db
touch future_ca_db/index.txt
echo 2000 > future_ca_db/serial
cat > future_ca_setup.cnf <<EOF
[ca]
default_ca = future_section
[future_section]
database = future_ca_db/index.txt
serial = future_ca_db/serial
new_certs_dir = future_ca_db
private_key = permissions_ca_key.pem
certificate = permissions_ca_cert.pem
default_md = sha256
default_days = 365
default_crl_days = 30
policy = pol
[pol]
commonName = supplied
organizationName = optional
countryName = optional
EOF
openssl ecparam -genkey -name prime256v1 -noout -out future_signer_key.pem
openssl req -new -key future_signer_key.pem \
    -subj "/CN=future-signer/O=ZeroDDS/C=DE" -out future_signer.csr
# Use 4-digit YYYYMMDDHHMMSSZ for "Generalized-Time" (post-2049).
openssl ca -batch -config future_ca_setup.cnf \
    -in future_signer.csr -out future_signer_cert.pem \
    -startdate 20990101000000Z -enddate 20991231235959Z -notext

# -------------------- RSA PKCS#1 v1.5 signer --------------------
openssl genrsa -out rsa_pkcs1_signer_key.pem 2048
openssl req -new -key rsa_pkcs1_signer_key.pem \
    -subj "/CN=rsa-pkcs1-signer/O=ZeroDDS/C=DE" -out rsa_pkcs1_signer.csr
openssl x509 -req -in rsa_pkcs1_signer.csr \
    -CA permissions_ca_cert.pem -CAkey permissions_ca_key.pem \
    -CAcreateserial -days 3650 -sha256 \
    -out rsa_pkcs1_signer_cert.pem

# -------------------- The Permissions XML to sign --------------------
cat > permissions.xml <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <permissions>
    <grant name="signed-alice">
      <subject_name>CN=alice</subject_name>
      <allow_rule>
        <publish><topic>Chatter</topic></publish>
        <subscribe><topic>Echo</topic></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
EOF

# -------------------- Sign with primary signer (opaque + multipart) -----------
# Opaque-signed (Cyclone-default): smime-type=signed-data, content embedded.
openssl cms -sign -in permissions.xml -text \
    -signer signer_ec_cert.pem -inkey signer_ec_key.pem \
    -out valid_permissions.smime.p7s -nodetach
# Multipart/signed (detached signature, RFC 5751 §3.4.2).
openssl cms -sign -in permissions.xml -text \
    -signer signer_ec_cert.pem -inkey signer_ec_key.pem \
    -out valid_permissions_multipart.smime.p7s
# Detached PEM PKCS#7 (paired with cleartext file).
openssl cms -sign -in permissions.xml \
    -signer signer_ec_cert.pem -inkey signer_ec_key.pem \
    -outform PEM -out valid_permissions_pem.p7s
cp permissions.xml valid_permissions_xml.txt

# -------------------- Wrong-CA --------------------
openssl cms -sign -in permissions.xml -text \
    -signer wrong_signer_cert.pem -inkey wrong_signer_key.pem \
    -out wrong_ca_permissions.smime.p7s -nodetach

# -------------------- Tampered: sign valid, then alter the embedded XML ------
openssl cms -sign -in permissions.xml -text \
    -signer signer_ec_cert.pem -inkey signer_ec_key.pem \
    -out tampered_permissions.smime.p7s -nodetach
# Mutate one byte inside the base64-encoded content (byte 800 chosen
# arbitrarily; the file is ~2 KB so 800 lands in the middle of the
# base64 payload).
python3 - <<'PY'
import pathlib
p = pathlib.Path("tampered_permissions.smime.p7s")
data = bytearray(p.read_bytes())
# Find first base64 line after the headers and flip a byte.
header_end = data.find(b"\n\n")
if header_end < 0:
    header_end = data.find(b"\r\n\r\n")
target = header_end + 50
data[target] = data[target] ^ 0x01
# Keep base64-decodable: only flip a byte that stays printable
# (toggle low bit; if it becomes invalid base64, openssl would still
# parse most layouts but the digest will mismatch — which is what we
# test).
p.write_bytes(bytes(data))
PY

# -------------------- Intermediate-CA chain ----------------------------------
# Embed intermediate cert in the PKCS#7 bundle via -certfile.
openssl cms -sign -in permissions.xml -text \
    -signer int_signer_cert.pem -inkey int_signer_key.pem \
    -certfile intermediate_ca_cert.pem \
    -out intermediate_chain.smime.p7s -nodetach

# -------------------- Expired / Future signers -------------------------------
openssl cms -sign -in permissions.xml -text \
    -signer expired_signer_cert.pem -inkey expired_signer_key.pem \
    -out expired_signer.smime.p7s -nodetach
openssl cms -sign -in permissions.xml -text \
    -signer future_signer_cert.pem -inkey future_signer_key.pem \
    -out future_signer.smime.p7s -nodetach

# -------------------- RSA PKCS#1 signer --------------------------------------
openssl cms -sign -in permissions.xml -text \
    -signer rsa_pkcs1_signer_cert.pem -inkey rsa_pkcs1_signer_key.pem \
    -out rsa_pkcs1_signer.smime.p7s -nodetach

# -------------------- Cleanup intermediate artefacts -------------------------
rm -rf expired_ca_db future_ca_db
rm -f *.csr *.srl expired_ca_setup.cnf future_ca_setup.cnf intermediate_v3.cnf

# Keep CA certs + permissions.xml as fixture inputs; signer privkeys are
# not needed at test-runtime but we keep them for re-generation. Strip
# them for repo hygiene if requested:
# rm -f *_key.pem

echo "Fixtures generated."
ls -la *.smime.p7s *.p7s permissions.xml *.pem 2>/dev/null

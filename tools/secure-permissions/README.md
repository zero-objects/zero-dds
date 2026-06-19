# zerodds-secure-permissions

CMS / PKCS#7 **signing** (and verification) of DDS-Security `governance` and
`permissions` XML for the ZeroDDS builtin AccessControl plugin.

DDS-Security 1.2 §9.4.1.3 requires both documents to be S/MIME (CMS `SignedData`)
signed by the **Permissions CA** — an unsigned `permissions.xml` is rejected by
any conformant participant. ZeroDDS's runtime
([`zerodds-security-permissions`](../../crates/security-permissions)) *verifies*
these documents; this tool *produces* them.

The output is an **opaque** S/MIME `application/pkcs7-mime; smime-type=signed-data`
document — a single self-contained `.p7s` that carries both the signed XML
(embedded `eContent`) and the signature. `sign` **self-verifies** the result
against the runtime verifier before writing, so any file it produces is
guaranteed to load in a ZeroDDS participant. The signature is ECDSA P-256 /
SHA-256 (ASN.1-DER).

The `.p7s` is **opaque S/MIME** (MIME headers + base64), **not** raw DER or PEM —
so `openssl` reads it with its default `-inform SMIME`:

```bash
openssl cms -verify -in permissions.p7s -CAfile perm_ca_cert.pem -inform SMIME
```

## Usage

```text
zerodds-secure-permissions sign   --signer-cert <perm_ca.pem> --signer-key <perm_ca_key.pem> \
                                  --in <doc.xml> --out <doc.p7s>
zerodds-secure-permissions verify --ca <perm_ca.pem> --p7s <doc.p7s> [--out <doc.xml>]
```

The same `sign` invocation applies to both `governance.xml` and
`permissions.xml`. `verify` prints a status line; pass `--out <file>` to write
the recovered XML.

> **Signer key must be PKCS#8** (`BEGIN PRIVATE KEY`). `openssl ecparam -genkey`
> emits a **SEC1** key (`BEGIN EC PRIVATE KEY`) — convert it first:
> `openssl pkcs8 -topk8 -nocrypt -in key.pem -out key_pkcs8.pem`. (The tool
> detects a SEC1 key and tells you this.)

## Issuing a Permissions CA + signing (full recipe)

```bash
# 1. Permissions CA (ECDSA P-256), PKCS#8 key.
openssl ecparam -genkey -name prime256v1 -noout -out ec.pem
openssl pkcs8 -topk8 -nocrypt -in ec.pem -out perm_ca_key.pem
openssl req -x509 -new -nodes -key perm_ca_key.pem -sha256 -days 3650 \
    -subj "/CN=ZeroDDS Permissions CA/O=ZeroDDS/C=DE" -out perm_ca_cert.pem

# 2. Sign governance + permissions.
zerodds-secure-permissions sign --signer-cert perm_ca_cert.pem --signer-key perm_ca_key.pem \
    --in governance.xml   --out governance.p7s
zerodds-secure-permissions sign --signer-cert perm_ca_cert.pem --signer-key perm_ca_key.pem \
    --in permissions.xml  --out permissions.p7s
```

Point a participant's [`SecurityProfileConfig`](../../crates/security-runtime)
at `perm_ca_cert.pem` (as `permissions_ca_pem`) plus `governance.p7s` /
`permissions.p7s`. **Rotation** = re-sign with a fresh `permissions.xml` (use a
short `not_after` on the grants for time-boxed validity); **revocation** of an
identity is handled by the Identity CA's CRL
([`zerodds-security-pki`](../../crates/security-pki)).

## Exit codes

| Code | Meaning                              |
| ---- | ------------------------------------ |
| `0`  | ok                                   |
| `1`  | signing / verification error         |
| `2`  | bad invocation                       |

## Tests

```bash
cargo test -p zerodds-secure-permissions   # round-trips through the real CmsPkcs7Verifier
```

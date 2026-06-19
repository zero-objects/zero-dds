<!-- SPDX-License-Identifier: Apache-2.0 -->
# Per-adapter DDS-Security identities (dds4iobroker integration guide)

This guide shows how to give **each adapter its own DDS participant with its own
X.509 identity**, so DDS-Security authentication and access control apply
*per adapter* — the goal of dds4iobroker fix-list item **#3**. It ties together
the signing tool from item **#2**
([`zerodds-secure-permissions`](../../tools/secure-permissions)) and the runtime
security profile API ([`zerodds-security-runtime`](../../crates/security-runtime)).

## Why per-adapter participants

In DDS-Security the security identity is bound to the **DomainParticipant**: the
IdentityCertificate authenticates the participant, and access control evaluates
*that participant's* permissions grants. If every RESP client shares **one** shim
participant, they share **one** identity — DDS-Security cannot tell them apart or
authorize them individually. Giving each adapter its own participant (own cert +
own grants) is what makes per-adapter enforcement possible.

This costs more participants (more SPDP/SEDP discovery, more certificates to
manage), so it is a real trade-off for very large adapter counts — but it is the
only model in which `governed`/`regulated` access control is meaningful per
adapter.

## PKI you need

Two CAs (they may belong to the same org but should use distinct keys):

| CA | Role |
| --- | --- |
| **Identity CA** | Signs each participant's identity certificate; authenticates participants in the handshake. |
| **Permissions CA** | Signs the `governance` and `permissions` documents (item #2). |

```bash
# Identity CA
openssl ecparam -genkey -name prime256v1 -noout -out id_ec.pem
openssl pkcs8 -topk8 -nocrypt -in id_ec.pem -out identity_ca_key.pem
openssl req -x509 -new -nodes -key identity_ca_key.pem -sha256 -days 3650 \
    -subj "/CN=ZeroDDS Identity CA/O=ZeroDDS/C=DE" -out identity_ca.pem

# Permissions CA
openssl ecparam -genkey -name prime256v1 -noout -out perm_ec.pem
openssl pkcs8 -topk8 -nocrypt -in perm_ec.pem -out permissions_ca_key.pem
openssl req -x509 -new -nodes -key permissions_ca_key.pem -sha256 -days 3650 \
    -subj "/CN=ZeroDDS Permissions CA/O=ZeroDDS/C=DE" -out permissions_ca.pem
```

## Per adapter: issue an identity certificate

Each adapter gets an EE certificate signed by the Identity CA. The **subject
name** is the handle that the permissions grants reference.

```bash
ADAPTER=adapter1
openssl ecparam -genkey -name prime256v1 -noout -out ${ADAPTER}_ec.pem
openssl pkcs8 -topk8 -nocrypt -in ${ADAPTER}_ec.pem -out ${ADAPTER}_key.pem
openssl req -new -key ${ADAPTER}_key.pem \
    -subj "/CN=${ADAPTER}/O=ZeroDDS/C=DE" -out ${ADAPTER}.csr
openssl x509 -req -in ${ADAPTER}.csr -CA identity_ca.pem -CAkey identity_ca_key.pem \
    -CAcreateserial -days 365 -sha256 -out ${ADAPTER}_cert.pem
```

## Sign governance + per-adapter permissions (item #2)

`governance.xml` is usually shared by all adapters; `permissions.xml` carries the
grants. Use one permissions document per adapter (cleanest isolation) or a shared
one with a `<grant>` per `subject_name`. Sign both with the **Permissions CA**:

```bash
zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
    --in governance.xml --out governance.p7s
zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
    --in adapter1_permissions.xml --out adapter1_permissions.p7s
```

`sign` self-verifies against the runtime verifier before writing — a `.p7s` that
lands on disk is guaranteed to load. The grant's `subject_name` must match the
adapter cert's subject (`CN=adapter1,O=ZeroDDS,C=DE`).

Two setup notes (common stumbles, not bugs):

- **Signer key must be PKCS#8** (`BEGIN PRIVATE KEY`). `openssl ecparam -genkey`
  produces a **SEC1** key (`BEGIN EC PRIVATE KEY`) — the `openssl pkcs8 -topk8
  -nocrypt` steps above already convert it. (The tool detects a SEC1 key and
  prints the conversion command.)
- The `.p7s` is **opaque S/MIME** (MIME headers + base64), not raw DER/PEM. To
  cross-check with OpenSSL use its default `-inform SMIME`:
  `openssl cms -verify -in permissions.p7s -CAfile permissions_ca.pem -inform SMIME`.

## Build the secured participant (per adapter)

The broker (`crates/resp-dds-server`, `dds` feature) links `zerodds-security-runtime`
+ `zerodds-dcps`. For each adapter, build a [`SecurityProfileConfig`] pointing at
that adapter's files, hand it to [`SecurityProfile::from_files`], and attach the
resulting gate to the runtime:

```rust
use std::sync::Arc;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security_runtime::{SecurityProfile, SecurityProfileConfig};

fn start_adapter_participant(domain_id: u32, adapter_id: &str) -> anyhow::Result<Arc<DcpsRuntime>> {
    let base = format!("pki/adapters/{adapter_id}");
    let cfg = SecurityProfileConfig {
        domain_id,
        identity_ca_pem:    "pki/identity_ca.pem".into(),
        identity_cert_pem:  format!("{base}/identity_cert.pem").into(),
        identity_key_pem:   format!("{base}/identity_key.pem").into(),
        permissions_ca_pem: "pki/permissions_ca.pem".into(),
        governance_p7s:     "pki/governance.p7s".into(),
        permissions_p7s:    format!("{base}/permissions.p7s").into(),
    };

    // A starting 16-byte GUID, unique per adapter (e.g. hash of the adapter id).
    let seed_guid: [u8; 16] = derive_seed_guid(adapter_id);

    // `from_files` (a) validates the identity cert against the Identity CA,
    // (b) verifies governance.p7s + permissions.p7s against the Permissions CA
    //     and parses the grants, and (c) returns a ready gate plus the
    //     DDS-Security §9.3.3-adjusted GUID — its prefix is cryptographically
    //     bound to the identity, so you MUST use it for the participant.
    let profile = SecurityProfile::from_files(&cfg, seed_guid)?;
    let prefix = GuidPrefix::from_bytes(
        profile.adjusted_participant_guid[..12].try_into().expect("12 bytes"),
    );

    let rt = DcpsRuntime::start(
        domain_id as i32,
        prefix,
        RuntimeConfig {
            security: Some(profile.gate),
            ..RuntimeConfig::default()
        },
    )?;
    Ok(rt)
}
```

Each adapter that calls this gets a **distinct authenticated identity**: the
handshake proves possession of that adapter's private key, and access control
evaluates that adapter's grants. An out-of-grant topic is denied for that
participant only.

## Rotation & revocation

- **Rotation**: re-sign a fresh `permissions.xml` (a short `not_after` on the
  grants makes them time-boxed) and/or re-issue the identity cert. No code
  change — the broker reloads the files and rebuilds the profile.
- **Revocation**: publish a CRL from the Identity CA; the PKI plugin
  ([`zerodds-security-pki`](../../crates/security-pki), `crl.rs`) rejects revoked
  identities at handshake time.

## Proof / references

- **Live multi-identity proof**: `crates/dcps/tests/security_live_e2e.rs` runs
  **three** participants in one process, each with its own identity cert, doing
  the real PKI handshake + crypto-token exchange over UDP (Linux host).
- **Signer ↔ verifier proof**: `tools/secure-permissions` round-trips its output
  through the real `CmsPkcs7Verifier` (and `openssl cms -verify`).
- **API**: [`SecurityProfileConfig`] / [`SecurityProfile::from_files`] in
  `crates/security-runtime/src/profile.rs`; `RuntimeConfig.security` in
  `crates/dcps/src/runtime.rs`.

[`SecurityProfileConfig`]: ../../crates/security-runtime/src/profile.rs
[`SecurityProfile::from_files`]: ../../crates/security-runtime/src/profile.rs

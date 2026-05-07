# Security

DDS-Security 1.2 runs as a set of pluggable services on top of
the standard DDS pipeline. ZeroDDS ships built-in implementations
of all five plugins.

| Plugin | OMG-spec | Crate |
|---|---|---|
| Authentication | §8.3 (PKI / Mutual-TLS-style 3-way handshake) | `zerodds-security-pki` |
| Access Control | §8.4 (Governance + Permissions XML) | `zerodds-security-permissions` |
| Cryptographic | §8.5 (AES-GCM 128/256) | `zerodds-security-crypto` |
| Logging | §8.6 (audit events) | `zerodds-security-logging` |
| Data Tagging | §8.7 (per-sample tags) | `zerodds-security` |

## Enable the security feature

Cargo:

```toml
[dependencies]
zerodds-dcps = { ..., features = ["security"] }
```

This pulls `zerodds-security-runtime` and exposes
`RuntimeConfig.security: Option<Arc<SharedSecurityGate>>`.

## What you supply

A complete security setup needs:

1. **Identity Certificate** + private key per participant (X.509
   PEM).
2. **Identity CA** certificate (the CA that issues identity certs).
3. **Permissions CA** certificate (the CA that signs permissions
   XML files).
4. **Governance XML** — domain-wide rules (which topics get which
   protection level).
5. **Permissions XML** — per-participant rules (who can publish /
   subscribe to which topic), signed by the Permissions CA.

Generate scaffolding:

```bash
# Generate a test CA hierarchy
openssl req -x509 -newkey rsa:4096 -nodes -keyout id_ca.key \
  -out id_ca.pem -days 365 \
  -subj "/CN=ZeroDDS Test Identity CA"

openssl req -x509 -newkey rsa:4096 -nodes -keyout perm_ca.key \
  -out perm_ca.pem -days 365 \
  -subj "/CN=ZeroDDS Test Permissions CA"

# Per-participant identity cert
openssl req -newkey rsa:2048 -nodes -keyout part1.key \
  -out part1.csr \
  -subj "/CN=Participant 01"
openssl x509 -req -in part1.csr -CA id_ca.pem -CAkey id_ca.key \
  -CAcreateserial -out part1.pem -days 90

# Sign Permissions XML with permissions CA
openssl smime -sign -text -in permissions.xml -out permissions.p7s \
  -signer perm_ca.pem -inkey perm_ca.key -outform DER -nodetach
```

## Governance XML

Domain-wide policy. Example: encrypt everything on domain 0:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="omg_shared_ca_governance.xsd">
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id>0</id>
      </domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>TRUE</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</discovery_protection_kind>
      <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>TRUE</enable_discovery_protection>
          <enable_liveliness_protection>TRUE</enable_liveliness_protection>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
```

## Permissions XML

Per-participant rules. Example:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="omg_shared_ca_permissions.xsd">
  <permissions>
    <grant name="Robot01">
      <subject_name>CN=Participant 01</subject_name>
      <validity>
        <not_before>2026-05-03T00:00:00</not_before>
        <not_after>2027-05-03T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>0</id></domains>
        <publish>
          <topics>
            <topic>Telemetry</topic>
          </topics>
        </publish>
        <subscribe>
          <topics>
            <topic>Commands</topic>
          </topics>
        </subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
```

The XML must be signed with the permissions CA — see the
`openssl smime` invocation above.

## Wire protection levels

Per the governance file, each topic gets one of:

| Kind | Effect |
|---|---|
| `NONE` | Plaintext (legacy) |
| `SIGN` | Authenticated, plaintext payload |
| `ENCRYPT` | Authenticated + confidential |
| `ENCRYPT_WITH_ORIGIN_AUTHENTICATION` | + per-receiver MAC |

ZeroDDS supports all four; `ENCRYPT` is the production default.

## Code: wiring up the gate

```rust
use std::sync::Arc;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_security_runtime::SharedSecurityGate;
use zerodds_security_permissions::parse_governance_xml;
use zerodds_security_crypto::AesGcmCryptoPlugin;

let governance = std::fs::read_to_string("governance.xml")?;
let gov = parse_governance_xml(&governance)?;
let crypto = Box::new(AesGcmCryptoPlugin::new());

let gate = Arc::new(SharedSecurityGate::new(0, gov, crypto));

let cfg = RuntimeConfig {
    security: Some(gate),
    ..Default::default()
};
let rt = DcpsRuntime::start(0, prefix, cfg)?;
```

After this, every outbound RTPS message gets SRTPS-wrapped per
the governance rules; every inbound message gets unwrapped or
dropped on policy violation.

## Heterogeneous deployments

Per-reader protection levels let one writer fan out to
peers with different protection requirements: legacy peer →
plaintext, modern peer → ENCRYPT, classified peer →
ENCRYPT_WITH_ORIGIN_AUTHENTICATION. The runtime keeps a
`reader_protection: BTreeMap<PeerKey, ProtectionLevel>` per writer
slot, populated from SEDP `security_info`.

## Reading further

- OMG DDS-Security 1.2 — formal/2018-04-01 — full normative.
- `docs/architecture/08_heterogeneous_security.md` — internal
  design notes (German, internal repo only).
- `crates/security-pki/README.md` — handshake details.
- `crates/security-crypto/README.md` — AES-GCM, HMAC, key
  derivation.

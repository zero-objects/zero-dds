# `zerodds-security`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security/badge.svg)](https://docs.rs/zerodds-security)

DDS-Security 1.1 (formal/2018-04-01) Plugin-SPI fuer den
[ZeroDDS](https://zerodds.org)-Stack: Trait-Definitionen, Token-
Datenmodell, Generic-Message-Topics. Pure-Rust + `alloc`. Safety
classification: **SAFE** (trust-neutraler SPI-Layer).

## Spec-Mapping

| Spec | Trait / Modul | Konkrete Impl |
|------|---------------|---------------|
| §8.3 Authentication | `AuthenticationPlugin` | `zerodds-security-pki` |
| §8.4 Access Control | `AccessControlPlugin` | `zerodds-security-permissions` |
| §8.5 Cryptographic | `CryptographicPlugin` | `zerodds-security-crypto` |
| §8.6 Logging | `LoggingPlugin` | `zerodds-security-logging` |
| §8.7 Data Tagging | `DataTaggingPlugin` | `zerodds-security-runtime` |

Coverage-Doc: `docs/spec-coverage/dds-security-1.2.md` (50 done / 0 partial / 0 open / 1 n/a, K6-Audit).

## Was ist drin

**Plugin-Traits** (object-safe, `Box<dyn Plugin>`-erasable):
- `AuthenticationPlugin` — Identity-Validation + Handshake.
- `AccessControlPlugin` — Permissions-Check, Topic-Allow-/Deny.
- `CryptographicPlugin` — Encrypt/Decrypt-Submessage + Key-Material + Receiver-Specific-MACs.
- `LoggingPlugin` — Audit-Events.
- `DataTaggingPlugin` — Built-in DataTagging (DDS-Security 1.2 §8.7).

**Token-Datenmodell:**
- `IdentityToken`, `PermissionsToken`, `CryptoToken`, `IdentityStatusToken`.
- `DataHolder`, `BinaryProperty`, `WireProperty`.

**Generic Messages** (DCPSParticipantStatelessMessage + DCPSParticipantVolatileMessageSecure):
- `ParticipantGenericMessage`, `MessageIdentity`.
- Topic-Konstanten: `TOPIC_STATELESS_MESSAGE`, `TOPIC_VOLATILE_MESSAGE_SECURE`, `TYPE_NAME_GENERIC_MESSAGE`.

**Querschnitt:**
- `Property`, `PropertyList` — Plugin-Konfiguration via `<participant_qos><property>`.
- `security_topic_qos` — Built-in-Security-Topic-QoS-Profile (§7.4.5).
- `SecurityError` — alle Plugin-Fehler.
- `mock` (Feature `std`) — Test-Mock-Plugins.

## Schichten-Position

Layer 4 — Core Services (SPI-Crate). Pure-Rust + `alloc`, **keine** ZeroDDS-Crate-Deps. Wird von 7 weiteren Security-Crates konsumiert (`security-pki`, `-crypto`, `-keyexchange`, `-permissions`, `-logging`, `-rtps`, `-runtime`) plus von `zerodds-discovery` (Built-in-Endpoint-Slots) und `zerodds-dcps` (Feature `security`).

## Quickstart

```rust,ignore
use zerodds_security::{AuthenticationPlugin, AccessControlPlugin};
use zerodds_security::mock::MockAuthenticationPlugin;

let auth: Box<dyn AuthenticationPlugin> = Box::new(MockAuthenticationPlugin::new());
// Use auth.validate_local_identity(...), auth.begin_handshake_request(...) etc.
```

Produktive Use-Cases bauen die echten Plugins (`security-pki`, etc.) und stecken sie via `Box<dyn Plugin>` in den DCPS-Participant.

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Mutex + Thread-Safe Mock |
| `alloc` | ✅ via std | `Vec`/`String` |
| `safety` | ❌ | Reserve-Hook |

## Stabilitaet

`1.0.0-rc.1` ist **API-frozen** — Breaking Changes erfordern v2.0-Major-Bump. Semver-Patch + Minor duerfen nur neue Methoden mit Default-Body oder non-breaking Enum-Varianten hinzufuegen. Diese Frozen-Pledge ist verbindlich, weil 7 Schwester-Crates + dcps + discovery von dem SPI abhaengen.

## Tests

```bash
cargo test -p zerodds-security
```

39 Unit-Tests + 1 Doc-Test grün.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- `docs/spec-coverage/dds-security-1.2.md` — Spec-Coverage-Doc.
- [`zerodds-security-pki`](../security-pki) — X.509 + RSA-PSS + ECDSA + OCSP/CRL Authentication.
- [`zerodds-security-crypto`](../security-crypto) — AES-GCM/HMAC Cryptographic Plugin.
- [`zerodds-security-permissions`](../security-permissions) — Governance + Permissions-XML.
- [`zerodds-security-rtps`](../security-rtps) — RTPS-Header-AAD-Wrapper.
- [`zerodds-security-runtime`](../security-runtime) — Plugin-Runtime + Built-in DataTagging.

# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security`-Crate (DDS-Security 1.1 Plugin-SPI).

### Spec-Referenzen

- **OMG DDS-Security 1.1** (formal/2018-04-01) §8.3 (Authentication), §8.4 (Access Control), §8.5 (Cryptographic), §8.6 (Logging), §8.7 (Data Tagging).
- **DDS-Security 1.2-Delta:** Built-in DataTagging.
- Coverage-Doc: `docs/spec-coverage/dds-security-1.2.md` (50 done / 0 partial / 0 open / 1 n/a, K6-Audit).

### Public-API

**Plugin-Traits:**
- `AuthenticationPlugin` (object-safe) — validate_local_identity, validate_remote_identity, begin_handshake_*, process_handshake, get_shared_secret, return_*.
- `SharedSecretHandle`, `AuthRequestMessageToken`, `AuthLookupBridge`.
- `AccessControlPlugin` — validate_local_permissions, check_create_*/check_remote_*-pfade, get_permissions_token, return_*.
- `CryptographicPlugin` — encrypt_submessage / decrypt_submessage / encrypt_serialized_payload / decrypt_serialized_payload mit Receiver-Specific-MACs, register_local_*/return_*-pfade, set_remote_participant_crypto_tokens.
- `CryptoHandle`, `ReceiverSpecificMac`.
- `LoggingPlugin` + `LogLevel`.
- `DataTaggingPlugin`.

**Token-Datenmodell:**
- `IdentityToken`, `PermissionsToken`, `CryptoToken`, `IdentityStatusToken`.
- `DataHolder`, `BinaryProperty`, `WireProperty`.

**Generic-Messages (Spec §7.4.3):**
- `ParticipantGenericMessage`, `MessageIdentity`.
- `TOPIC_STATELESS_MESSAGE`, `TOPIC_VOLATILE_MESSAGE_SECURE`, `TYPE_NAME_GENERIC_MESSAGE`.

**Querschnitt:**
- `Property`, `PropertyList`.
- `security_topic_qos` (§7.4.5).
- `SecurityError`.
- `mock::*` (Feature `std`) — Test-Mocks.

### Implementierung

Trait-basierter SPI mit `Box<dyn Plugin>`-Erasure: alle 5 Plugin-Traits sind object-safe, sodass Backends (rustls vs. ring vs. mbedtls) ohne Crate-Wiring austauschbar sind. Jeder Trait ist in sich geschlossen — keine Cross-Trait-Generics — damit Erweiterungen in einem Plugin nicht andere brechen.

`mock::MockAuthenticationPlugin` + `MockAccessControlPlugin` + `MockCryptographicPlugin` sind nicht-produktive Test-Adapter, dokumentiert als "do not use in production" und nur unter `cfg(feature = "std")` gebaut. Produktions-Plugins leben in `zerodds-security-pki/-crypto/-keyexchange/-permissions/-logging/-rtps/-runtime`.

Generic-Message-Encoding ist XCDR2-Final per Spec §7.4.3.4. Topic-Konstanten matchen byte-genau die Spec-Tabelle 7-30 / 7-31.

`forbid(unsafe_code)` ist gesetzt; `Box<dyn Plugin>` ist per Modul-Doc als bewusste SPI-Architektur-Entscheidung dokumentiert (zerodds-lint `no_dyn_in_safe`-Allow).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** keine ZeroDDS-Crate-Deps. Pure-Rust + `alloc`.
- **Dependents (out):** `zerodds-security-pki`, `-crypto`, `-keyexchange`, `-permissions`, `-logging`, `-rtps`, `-runtime`; `zerodds-discovery` (Built-in-Endpoint-Slots fuer Security-Topics); `zerodds-dcps` (Feature `security`).
- **Feature-Flags:** `std` (default), `alloc` (via std), `safety` (Reserve).

### Stabilitaet

**API-frozen ab 1.0.0-rc.1.** Breaking Changes erfordern v2.0-Major-Bump. Semver-Patch/Minor duerfen nur additive Erweiterungen (neue Methoden mit Default-Body, non-breaking Enum-Varianten) einfuegen. Diese Pledge ist verbindlich — 7 Schwester-Crates + dcps + discovery haengen davon ab.

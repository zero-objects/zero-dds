# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security` crate (DDS-Security 1.1 plugin SPI).

### Spec references

- **OMG DDS-Security 1.1** (formal/2018-04-01) §8.3 (Authentication), §8.4 (Access Control), §8.5 (Cryptographic), §8.6 (Logging), §8.7 (Data Tagging).
- **DDS-Security 1.2 delta:** built-in DataTagging.
- Coverage doc: `docs/spec-coverage/dds-security-1.2.md` (50 done / 0 partial / 0 open / 1 n/a, K6 audit).

### Public API

**Plugin-Traits:**
- `AuthenticationPlugin` (object-safe) — validate_local_identity, validate_remote_identity, begin_handshake_*, process_handshake, get_shared_secret, return_*.
- `SharedSecretHandle`, `AuthRequestMessageToken`, `AuthLookupBridge`.
- `AccessControlPlugin` — validate_local_permissions, check_create_*/check_remote_* paths, get_permissions_token, return_*.
- `CryptographicPlugin` — encrypt_submessage / decrypt_submessage / encrypt_serialized_payload / decrypt_serialized_payload with receiver-specific MACs, register_local_*/return_* paths, set_remote_participant_crypto_tokens.
- `CryptoHandle`, `ReceiverSpecificMac`.
- `LoggingPlugin` + `LogLevel`.
- `DataTaggingPlugin`.

**Token data model:**
- `IdentityToken`, `PermissionsToken`, `CryptoToken`, `IdentityStatusToken`.
- `DataHolder`, `BinaryProperty`, `WireProperty`.

**Generic messages (spec §7.4.3):**
- `ParticipantGenericMessage`, `MessageIdentity`.
- `TOPIC_STATELESS_MESSAGE`, `TOPIC_VOLATILE_MESSAGE_SECURE`, `TYPE_NAME_GENERIC_MESSAGE`.

**Cross-cutting:**
- `Property`, `PropertyList`.
- `security_topic_qos` (§7.4.5).
- `SecurityError`.
- `mock::*` (feature `std`) — test mocks.

### Implementation

Trait-based SPI with `Box<dyn Plugin>` erasure: all 5 plugin traits are object-safe, so backends (rustls vs. ring vs. mbedtls) are swappable without crate wiring. Each trait is self-contained — no cross-trait generics — so extensions in one plugin do not break others.

`mock::MockAuthenticationPlugin` + `MockAccessControlPlugin` + `MockCryptographicPlugin` are non-production test adapters, documented as "do not use in production" and only built under `cfg(feature = "std")`. The production plugins live in `zerodds-security-pki/-crypto/-keyexchange/-permissions/-logging/-rtps/-runtime`.

Generic-message encoding is XCDR2-final per spec §7.4.3.4. Topic constants match the spec table 7-30 / 7-31 byte-exactly.

`forbid(unsafe_code)` is set; `Box<dyn Plugin>` is documented in the module docs as a deliberate SPI architecture decision (zerodds-lint `no_dyn_in_safe` allow).

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** no ZeroDDS crate deps. Pure Rust + `alloc`.
- **Dependents (out):** `zerodds-security-pki`, `-crypto`, `-keyexchange`, `-permissions`, `-logging`, `-rtps`, `-runtime`; `zerodds-discovery` (built-in endpoint slots for security topics); `zerodds-dcps` (feature `security`).
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserved).

### Stability

**API-frozen as of 1.0.0-rc.1.** Breaking changes require a v2.0 major bump. Semver patch/minor may only add additive extensions (new methods with a default body, non-breaking enum variants). This pledge is binding — 7 sibling crates + dcps + discovery depend on it.

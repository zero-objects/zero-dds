# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `SubjectNameForm` (`rfc4514`, `x509_name_oneline`, `xn_flag_oneline`): the form of the
  subject names in the IdentityToken and the PermissionsToken.

### Added (DDS-Security 1.2 §9.8, §11)

- `MockAccessControlPlugin::with_participant_config`, `with_topic_config` and
  `with_endpoint_config` change the configs the mock returns; its default
  `ParticipantSecurityConfig` does not restrict the algorithms.
- `MockAuthenticationPlugin::calls` lists the operations called on it; its
  handshake tokens and the AuthenticatedPeerCredentialToken are DataHolders
  as the stateless message carries them.

### Changed (DDS-Security 1.2 §7)

- BREAKING (property names): `zerodds.crypto.{rtps_origin_authentication,metadata_protection_kind,data_protection_kind}` are `org.zerodds.crypto.*` (§7.3.2).
### Added (DDS-Security 1.2 §9.6, §11)

- `LoggingPlugin` has `set_log_options`, `enable_logging` and `set_listener`
  (Table 46); `LogOptions`, `LoggerListener`; `LogLevel::Trace`, the constants
  `FATAL_LEVEL` to `TRACE_LEVEL` and `LogLevel::{to_spec_level, from_spec_level,
  spec_name}` (§9.6.2.1.1).
- `idl/dds_security.idl`, the IDL representation of the plugin interfaces
  (§11.2), and a test that renders and compiles the C, C++ classic, C++11, Java
  classic and Java modern representations (§11.3 to §11.7).


### Changed

- `AccessControlPlugin` has the 29 operations of DDS-Security 1.2 Table 39 with the
  parameters of the spec (`PermissionsHandle`, `AccessDecision`,
  `ParticipantRequest`, `TopicRequest`, `EndpointRequest`, `RemoteReaderDecision`),
  `AccessControlListener` (`on_revoke_permissions`), the config types
  `ParticipantSecurityConfig`, `TopicSecurityConfig`, `EndpointSecurityConfig`
  with the masks of Tables 34, 35, 38, 67 and 69, plus `check_relay` (§7.1.1.4,
  default: refuse) and `plugin_class_id`. The former own operations
  `check_*_in_partitions` and `check_remote_*_match` are gone; implementors of the
  trait need the new signatures. `MockAccessControlPlugin` records the calls.
- `AuthenticationPlugin` carries the operations of DDS-Security 1.2 §9.3.2.11
  Table 29 and nothing else: `validate_local_identity(domain_id,
  participant_qos, candidate_participant_guid)` returns the
  `adjusted_participant_guid` (`LocalIdentityValidation`);
  `validate_remote_identity` returns `ValidationResult` with the
  `local_auth_request_token` (`RemoteIdentityValidation`);
  `begin_handshake_request` and `begin_handshake_reply` take
  `serialized_local_participant_data`; `HandshakeStepOutcome` has
  `SendFinalMessage` (VALIDATION_OK_FINAL_MESSAGE) and `PendingRetry`
  (VALIDATION_PENDING_RETRY); new operations `get_identity_status_token`,
  `set_participant_security_config`, `set_permissions_credential_and_token`,
  `get_authenticated_peer_credential_token`, `set_listener` and the six
  `return_*`. `get_identity_token` takes `&mut self`.
- `peer_permissions`, `peer_identity_certificate`, `local_auth_request_token`,
  `algorithm_masks` and `set_local_participant_data` are gone from the trait:
  `c.perm` and `c.id` are in the AuthenticatedPeerCredentialToken
  (`get_authenticated_peer_credential_token`), the AuthRequestMessageToken is an
  out parameter of `validate_remote_identity`, the algorithm masks are the
  result of `set_participant_security_config`, the participant data is the
  parameter `serialized_local_participant_data`. The operations outside
  Table 29 that remain are in the supertrait `AuthenticationExtensions`
  (`plugin_class_id`, `set_algo_nul_terminate`,
  `set_remote_message_auth_supported`, `set_peer_algorithm_names`,
  `get_permissions_token`, `check_identities`).

### Added

- `ValidationResult` (Table 30), `AuthenticationListener` (Table 31),
  `AuthStatusKind` (Table 32), `ParticipantSecurityConfig`, `AlgorithmNames`,
  `IssuedTokens`, `IdentityHandle::NIL` and the other nil handles.
- Module `cryptographic`: the interfaces `CryptoKeyFactory`, `CryptoKeyExchange`, `CryptoTransform` of DDS-Security 1.2 §9.5.1 Tables 42–44 with all parameters, the handle types, `SecureSubmessageCategory`, `ParticipantSecurityConfig`, `EndpointSecurityConfig`, the algorithm info structs; `VendorFormCrypto` for what the specification does not describe.

### Added (DDS-Security 1.2, §7 package)

- `BinaryProperty.propagate` (not serialized) and binary entries in
  `PropertyList` (§7.3.3); `AlgoNameNul` (the form of the algorithm names of a
  handshake token, §7.3.3.1).


### Fixed

- `SecurityError::code` and `minor_code` (DDS-Security 1.2 §9.2.1 Table 28:
  message, code, minor_code); `SecurityErrorKind::code`.
- Module `algorithm`: `CryptoAlgorithmId`, `CryptoAlgorithmBit`,
  `CryptoAlgorithmSet`, `CRYPTO_ALGORITHM_*`, `CryptoTransformKeyRevision`
  with its int holder, and the `CNAME_*`, `CID_*`, `CBIT_*` constants of §8.1
  to §8.3.
- `AuthenticationPlugin::set_remote_message_auth_supported` (default: no-op).
- `AuthenticationPlugin::algorithm_masks` and `AlgorithmMasks` (DDS-Security
  1.2 §7.3.11, §7.3.12): the signature and key establishment algorithms a
  plugin supports and requires (default: none announced).
- Removed: `ReceiverMac`, `CryptographicPlugin::encrypt_submessage_multi` and
  `decrypt_submessage_with_receiver_mac` (no mechanism of the specification,
  no caller in the runtime).

- `CryptographicPlugin::create_endpoint_crypto_token_for` (DDS-Security 1.2
  §10.5.3.1; default: the token of the endpoint) and the property
  `PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION` of `register_local_participant`.

- `CryptographicPlugin::encode_rtps_message_psk` and
  `decode_rtps_message_psk` (DDS-Security 1.2 §10.9.3.3.3; default: not
  implemented).

- `CryptographicPlugin::unregister_local_endpoint` and
  `unregister_matched_remote_participant` (default: nothing to release).

- `ParticipantGenericMessage::message_class_id` is `string<>`, unbounded
  (DDS-Security 1.2 §7.3.22); a class id over 256 octets was refused.

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

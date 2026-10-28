# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `org.zerodds.sec.origin_authentication_fallback` / `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK`
  (`spec`, `accept`), `SecurityProfileConfig.origin_authentication_fallback`,
  `VendorFormSettings.origin_authentication_fallback` and
  `SharedSecurityGate::set_origin_authentication_fallback`: the gate hands `accept` to the
  crypto plugin as `org.zerodds.crypto.origin_authentication_fallback` when it registers the
  local participant. Default `spec`, the refusal of DDS-Security 1.2 §10.5.3.3 Table 80.

- `SharedSecurityGate::set_algorithm_names`: `org.zerodds.sec.algorithm_names` also names the
  algorithm of `dds.perm_ca.algo` in the PermissionsToken of the builtin AccessControl
  plugin, as it does for the IdentityToken and the handshake.

- `org.zerodds.sec.subject_name_form` / `ZERODDS_SEC_SUBJECT_NAME_FORM`
  (`rfc4514`, `x509_name_oneline`, `xn_flag_oneline`), `SecurityProfileConfig.subject_name_form`
  (new field: struct literals need it), also for `zerodds_runtime_create_secure`.
  Fast DDS 3.6 compares `dds.perm_ca.sn` with `X509_NAME_oneline`, OpenDDS
  `dds.ca.sn` with `XN_FLAG_ONELINE`.

### Added (DDS-Security 1.2 §9.8)

- `profile::governance_from_files`, `builtin_crypto_plugin` and
  `gate_with_crypto`: the parts of `SecurityProfile::from_files`, for a runtime
  whose Authentication and Cryptographic plugins the application supplies.

- `builtin_attributes::kinds_of_endpoint_config`, `builtin_endpoint_kinds_asked`
  and `builtin_topic_name`: with an AccessControl plugin of the application the
  Metadata and Data Protection Kind of an endpoint follow its
  EndpointSecurityConfig, and the plugin is asked for the secure builtin
  endpoints (§9.8.4, §9.8.6 steps 6 and 10).
- `register_local_participant` gets the ParticipantSecurityConfig of the
  AccessControl plugin; the volatile message endpoints carry the property
  `dds.sec.builtin_endpoint_name` (§9.8.1 step 11, §9.8.9.1).

- The `rtps_psk_protection_kind` of the governance decides the protection of the RTPS messages with the pre-shared key (`rtps_psk_from_properties`); `key_revision_enabled`; `endpoint_required_mask` (the `algorithm_info` the plugin returned for an endpoint).

### Changed (DDS-Security 1.2 §7)

- BREAKING (property names): `dds.sec.crypto.maxblockspersession` is `org.zerodds.sec.crypto.max_blocks_per_session`; `dds.sec.crypto.rtps_psk_secret_passphrase_{alt,extra}` are `org.zerodds.sec.crypto.*` (§7.3.2).
### Added (DDS-Security 1.2 §9.7, §9.8, §10.6)

- `BuiltinDataTaggingPlugin` reports `DDS:Tagging:DDS_Discovery`; a
  `SecurityBundle` carries a DataTagging plugin.
- `SharedSecurityGate::check_create_topic`, `check_remote_topic`,
  `register_volatile_message_endpoints` (property `dds.sec.builtin_endpoint_name`).

### Fixed

- The endpoints of the two log topics follow the builtin rule of §10.6 for
  their key material, their announced mask and read/write protection.


### Changed

- `SharedSecurityGate` decides every check through a `Box<dyn AccessControlPlugin>`
  (`set_access_control` for a plugin of the application) and hands the
  configs, tokens and revocations of the plugin to the runtime.
  `SecurityGateError::AccessControl` carries the refusal and its minor code.
  `SecurityProfile` carries the signed documents and the identity certificate.
- `SecurityProfile` validates the identity with `validate_identity`: the
  `adjusted_participant_guid` is the one the plugin returns
  (DDS-Security 1.2 §10.3.3 Table 59), and the permissions document goes in with
  `set_permissions_credential_and_token`.
- `zerodds.sec.auth.algorithm_names` (`auto`, `spec`, `legacy`):
  `algorithm_naming_from_properties`.
- The `password` property does not apply to a `pkcs11:` key
  (`profile::private_key_pem`, DDS-Security 1.2 §10.3.1 Table 50).
- `SharedSecurityGate` calls only the operations of Tables 42–44 (`gate_crypto.rs`); `revise_local_entity_keys`, `activate_key_revision`, `install_remote_endpoint_token_from`; `transform_kx_*` removed. Protection of whole RTPS messages with a pre-shared key (`psk_gate.rs`, `rtps_psk_from_properties`, `set_rtps_psk`). Setting `org.zerodds.sec.algorithm_names`.

### Added (DDS-Security 1.2, §7 package)

- `plugin_attributes` (the masks `plugin_*_security_attributes` come from the
  plugin and are only passed on, §7.3.24), `builtin_attributes` (Table 21 and
  Table 17), `vendor_form_settings_from_env` and `AlgoNameNul`.
- `trust_chain.required_mask` holds the signature algorithm of the local
  permissions document (§7.3.11).

### Changed

- The ZeroDDS property names `zerodds.sec.*` and
  `dds.sec.{auth,access,crypto}.plugin_class` of `caps_wire` are
  `org.zerodds.sec.*` (§7.3.2); `dds.sec.data_tags.*` is gone, the tags go as
  `PID_DATA_TAGS`.


### Added

- `SharedSecurityGate::renew_local_participant_key`: a new revision of the key
  of the local participant (DDS-Security 1.2 §9.3.2.12.2).
- `SharedSecurityGate::register_local_endpoint_with_properties`: the
  `dds.sec.crypto.symmetric_cipher_algorithm` of an endpoint's own
  `PropertyQosPolicy` (DDS-Security 1.2 §7.3.21, §10.5.1.1).

### Fixed

- `ZERODDS_SEC_KX_KEY_FALLBACK` (`spec`, `accept`) and
  `ZERODDS_SEC_COMPATIBILITY_CHECK` (`spec`, `ignore`) give the two
  vendor-form settings their value when the property list has none; the
  property wins (`profile_config_from_properties_with_env`).
- `zerodds.sec.kx_key_fallback` (`spec`, the default, or `accept`) and
  `SharedSecurityGate::{set_,}kx_key_fallback`.
- `dds.sec.auth.key_establishment_algorithm` takes `AUTO` (ECDHE-CEUM+P256,
  which every vendor runs) and `DHE+MODP-2048-256` (DDS-Security 1.2 §10.3.1
  Table 50, §8.3 Table 26).
- `zerodds.sec.compatibility_check` (`spec`, the default, or `ignore`) and
  `SharedSecurityGate::{set_,}symmetric_cipher_bits`,
  `{set_,}compatibility_checks` for the checks of DDS-Security 1.2 §7.3.10 to
  §7.3.24 in the DCPS runtime.
- Removed: `SharedSecurityGate::transform_outbound_group`,
  `transform_inbound_group` and `peer_key_to_id` (HMAC-SHA256 receiver MACs, no
  mechanism of the specification, no caller in the runtime).
- `GovernancePolicyEngine::with_defaults` has the suite `Aes256Gcm`.

- `SecurityProfileConfig::symmetric_cipher_bits` from
  `dds.sec.crypto.symmetric_cipher_algorithm` (DDS-Security 1.2 §10.5.1
  Table 71); the key width of the crypto plugin was fixed at 256 bits. A
  Protection Kind `SIGN_WITH_ORIGIN_AUTHENTICATION` gets a GMAC key as `SIGN`
  does (it got GCM).

- `SharedSecurityGate::create_endpoint_token_for` and `local_token_for` make
  the token of an endpoint or participant for one remote endpoint or
  participant, with its receiver-specific key under origin authentication
  (DDS-Security 1.2 §10.5.3.1); `metadata_origin_authenticated`,
  `discovery_origin_authenticated`, `liveliness_origin_authenticated` and
  `rtps_origin_authenticated` read the `*_WITH_ORIGIN_AUTHENTICATION` kinds
  that `ProtectionLevel` collapses.

- `psk_crypto_config_from_properties` reads the properties of Table 86 of
  DDS-Security 1.2 (`rtps_psk_secret_passphrase`, `_alt`,
  `rtps_psk_symmetric_cipher_algorithm`).

- `zerodds.sec.auth.ocsp_unreachable` (`fail` default, `accept`) and
  `zerodds.sec.auth.ocsp_timeout_ms` (default 2000) for the OCSP check of the
  identity certificates (DDS-Security 1.2 §10.3.3 Table 59).

- `SharedSecurityGate::forget_remote` releases the peer's keys in the plugin
  and its permissions document; `unregister_local_endpoint` is new.

- `dds.sec.auth.private_key` takes `file:key.pem?password=…`; the query is
  the password, not part of the path. A password next to a `pkcs11:` private
  key is not applied and no longer refused (DDS-Security 1.2 §10.3.1 Table 50).
  `dds.sec.auth.password` is the Base64 AES-128 key of the clause by default;
  the OpenSSL-passphrase reading of Cyclone DDS and Fast DDS is the setting
  `zerodds.sec.auth.password_reading=openssl_passphrase`. Before, both
  readings applied. A key that needs the passphrase reading is refused with
  a message that names the setting.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-runtime` crate.

### Spec references

- **OMG DDS-Security 1.1** §8.5.3, §9.5.
- **OMG DDS-Security 1.2** §8.7 (data tagging).
- **ZeroDDS architecture §09** heterogeneous mesh + delegation.

### Public API

- `SecurityGate`.
- `engine::{GovernancePolicyEngine, PolicyEngine}`.
- `policy::{PolicyDecision, OutboundDecision, InboundVerdict}`.
- `caps::{PeerCapabilities, PeerCapabilitiesCache, ProtectionLevel, CapabilityWindow}`.
- `caps_wire::{encode_peer_capabilities, decode_peer_capabilities}`.
- `peer_class::{PeerClassMatch, CidrPattern}`.
- `endpoint::*`.
- `data_tagging::DataTaggingDefault`.
- `builtin_topics::*`.
- `anti_squatter::*`.
- `gateway_bridge::GatewayBridge`.
- `shared::*` shared verdict types.

### Implementation

`SecurityGate` holds the governance + crypto plugin as mut-refs and exposes `encode_outbound`/`decode_inbound`. `GovernancePolicyEngine` searches the `<topic_access_rule>` list for a domain-topic combination and returns a `PolicyDecision` with suite + protection kind + receiver-MAC set.

`PeerCapabilitiesCache` holds, per peer GUID, the last-seen capabilities, the offered protection level, a validity window, and optionally a `DelegationChain`. SPDP wire codec via the `caps_wire` module.

`peer_class::*` does the `<peer_class>` match for heterogeneous-mesh setups (vehicle ↔ C4I backend) — CIDR patterns, subject patterns, profile lookup.

`data_tagging::DataTaggingDefault` is the built-in DataTaggingPlugin impl (spec 1.2 §8.7).

`anti_squatter` implements spec §8.5.3: a replier must be registered at the ENT endpoint, otherwise reject.

`gateway_bridge::GatewayBridge` is the edge ↔ backend hop for the ZeroDDS heterogeneous mesh.

`forbid(unsafe_code)`.

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** all 7 security sibling crates + `zerodds-rtps` + `zerodds-qos`.
- **Dependents (out):** `dcps` (feature `security`), end-user builds.
- **Feature flags:** `std` (default).

### Stability

Public API + peer-caps SPDP mapping + data-tagging wire RC1-stable.

# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `PROP_ORIGIN_AUTHENTICATION_FALLBACK` (`org.zerodds.crypto.origin_authentication_fallback`,
  `spec` or `accept`), read by `register_local_participant`. Under an RTPS or Metadata
  Protection Kind with origin authentication, `spec` (the default) refuses an RTPS message or
  submessage under a key without a receiver-specific key, as Table 80 `decode_rtps_message`
  asks; `accept` verifies it with the common MAC alone, as Table 80 does for a key whose
  receiver_specific_key_id is zero. A key that carries a receiver-specific key is held to it
  under either value. The form of OpenDDS 3.34, which makes no receiver-specific keys.

### Fixed

- `decode_serialized_payload` opens a payload sealed with a key material the writer
  reuses for metadata and data under origin authentication (Table 78 allows the
  reuse; the first element then carries the receiver-specific key). The check of the
  expected kind refused it, because it compared the receiver-specific key with
  "no origin authentication". A payload has no receiver-specific MACs (Table 80
  `encode_serialized_payload`); that part of the key is not checked for it.
  Measured: Fast DDS 3.6 reuses the key when both are encrypted, and every sample of
  the oa-enc profile reached the ZeroDDS reader as the CryptoContent.

### Changed (DDS-Security 1.2 §7)

- BREAKING (property names): `zerodds.psk.{protocol_version,vendor_id,guid_prefix}` are `org.zerodds.psk.*` (§7.3.2).


- `AesGcmCryptoPlugin` and `PskCryptoPlugin` implement the interfaces of Tables 42–44 (`plugin/spec.rs`, `psk_rtps.rs`): key revisions, receiver lists with index, `preprocess_secure_submsg`, expected kind, `return_crypto_tokens`, HeaderExtension with `messageLength` and `messageChecksum`, random start of the IV suffix. `rsa_pss::verify_rsassa_pss_sha256` reads the salt length from the signature (§8.2 Table 25). Settings `org.zerodds.crypto.domain_id`, `.domain_tag`, `.rtps_additional_authenticated_data`.

### Changed (DDS-Security 1.2, §7 package)

- A remote key is found by (sender, `transformation_key_id`, revision)
  (§7.3.20.2); SEC_BODY, SEC_PREFIX and SEC_POSTFIX of an invalid length or
  position are refused (§7.4.6).


### Fixed

- `AesGcmCryptoPlugin::new`, `PskDerivedCryptoPlugin::new` and
  `Suite::default` are AES256-GCM (DDS-Security 1.2 §10.5.1.1); the default
  `transformation_kind` on the wire is `{0,0,0,4}`, it was `{0,0,0,2}`.
- Removed: `encrypt_submessage_multi`, `decrypt_submessage_with_receiver_mac`
  and `derive_session_hmac_key` (HMAC-SHA256 receiver MACs, no mechanism of
  the specification, no caller in the runtime).

- `dds.sec.crypto.symmetric_cipher_algorithm` (DDS-Security 1.2 §10.5.1
  Table 71, §10.5.1.1): `symmetric_cipher_key_bits` reads `AUTO` and the
  names of Table 22 (`AES128+GCM` and `AES128-GCM`, GMAC likewise); a
  missing or `AUTO` value is AES256-GCM. `AesGcmCryptoPlugin::new` is
  AES256-GCM (it was AES128-GCM), a DataWriter's or DataReader's property
  in the properties of `register_local_endpoint` overrides the width of the
  participant, `Suite::with_key_bits` gives the suite of the width.

- Origin authentication (DDS-Security 1.2 §10.5.2.1.1, §10.5.3.3.3,
  §10.5.3.3.6): the keymat reads `receiver_specific_key_id` and
  `master_receiver_specific_key` (zero if and only if empty, the length of the
  sender key); a key from a token with a receiver-specific key is checked on
  every submessage and RTPS message (the footer must hold the MAC of that key
  or the message is refused); a local key whose Metadata Protection Kind is a
  `*_WITH_ORIGIN_AUTHENTICATION` kind (or whose participant has the property
  `zerodds.crypto.rtps_origin_authentication`) makes a receiver-specific key
  for each remote endpoint (`create_endpoint_crypto_token_for`) and appends
  the MAC of each to its footers: `SessionReceiverSpecificKey :=
  HMAC256(MasterReceiverSpecificKey, "SessionReceiverKey" | MasterSalt |
  SessionId)`, AES-GMAC on the `common_mac` with the IV of the header. The
  keymat of every token had a zero receiver-specific key and the footers no
  MAC. `encrypt_submessage_multi` and `decrypt_submessage_with_receiver_mac`
  (an HMAC-SHA256 over the ciphertext, no mechanism of the specification) are
  unchanged and not used by the runtime.

- The `transformation_kind` of a CryptoHeader is the key revision (three
  octets) and the algorithm id (DDS-Security 1.2 §7.3.18):
  `CryptoTransformIdentifier` carries `key_revision`, a key material keeps the
  revision of its keymat, and a message under another revision of the same
  key id is refused. The two codecs read the octets differently, and the
  second refused any revision.

- `PskCryptoPlugin` is `DDS:Crypto:PSK` (DDS-Security 1.2 §10.9, Tables 86 to
  89, §10.5.2.1.3): the key material is derived from the passphrase as the
  spec's worked example prints it (SenderKeyId, `master_salt`,
  `master_sender_key`, revision from the passphrase id), `encode_rtps_message`
  writes the SRTPS_PREFIX with the flags A and P and the RTPS Header and
  HeaderExtension as AAD, `decode_rtps_message` picks the primary or an
  alternative passphrase by the CryptoHeader and derives the key of the
  sender from its RTPS Header, and the other operations do nothing. The
  earlier plugin of this name, with its own HKDF derivation, is
  `PskDerivedCryptoPlugin` (`ZeroDDS:Crypto:PSK-Derived:1.0`). A key
  material carries a `transformation_key_revision`, written into the
  CryptoHeader.

- `unregister_local_endpoint` and `unregister_matched_remote_participant`
  release the key material of an endpoint and of a remote participant
  (DDS-Security 1.2 §9.5.1.8).

- A protected SerializedPayload whose CryptoFooter announces
  `receiver_specific_macs` is refused; the count was not read (DDS-Security
  1.2 §10.5.3.3.1 Table 80).
- A CryptoHeader whose `transformation_kind` (key revision and algorithm id)
  differs from that of the key it selects is refused, for submessages,
  payloads and SRTPS (DDS-Security 1.2 §7.3.20.1).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-crypto` crate (`CryptographicPlugin` impl).

### Spec references

- **OMG DDS-Security 1.1** (formal/2018-04-01) §8.5 (CryptographicPlugin trait), §9.5 (built-in crypto plugin), §10.5 (wire format Tab.73/74/79).
- **OMG DDS-Security 1.2** delta — KeyMaterial AES-GCM-GMAC.

### Public API

- `AesGcmCryptoPlugin::{new, with_suite, with_shared_secret_provider}`.
- `PskCryptoPlugin::{new, with_class_id}`, `CLASS_ID_PSK_CRYPTO`, `HKDF_INFO_PSK_MASTER_KEY`.
- `Suite::{Aes128Gcm, Aes256Gcm, HmacSha256}` + `transform_kind_id`/`from_transform_kind_id`/`key_len`.
- `crypto_transform::{CryptoHeader, CryptoFooter, CryptoTransformIdentifier, CryptoTransformKind, BUILTIN_CRYPTO_PLUGIN, negotiate_transform}`.
- `session_key::{derive_session_key, derive_session_hmac_key, compute_aad, SESSION_KEY_TAG, SESSION_RECEIVER_KEY_TAG, AAD_HEADER_LEN}`.
- `aes_gcm_hw::{Arch, HwCapabilities, report}`.
- `metrics::CryptoOp` (feature `metrics`).

### Implementation

`AesGcmCryptoPlugin` holds an `RwLock<BTreeMap<CryptoHandle, KeyMaterial>>`. Each `KeyMaterial` slot carries suite + 4-byte transformation_key_id + master_key (16/32 bytes) + 32-byte master_salt + 4-byte session_id + AtomicU64 counter. Encrypt/decrypt use `derive_session_key(master_key, master_salt, session_id)` as the per-submessage AES key + `compute_aad(transform_kind, key_id, session_id, extension)` as the AES-GCM AAD — the hot path is spec-byte-compatible with Cyclone DDS and FastDDS.

`SharedSecretProvider` integration (PKI ↔ crypto): if a provider is registered via `with_shared_secret_provider`, `register_matched_remote_*` is derived deterministically via HKDF-SHA256 from the SharedSecret — both partners compute the same master key without a token exchange.

`PskCryptoPlugin` is a deterministic shared-secret plugin for out-of-band setups (e.g. unattended embedded). Setup token = HKDF(class_id `"DDS:Auth:PSK:1.0"` + setup_salt + identity_hash). Both partners derive the same master key.

`CryptoOp` (feature `metrics`) is a RAII span — on `Drop` it updates `dds_security_crypto_operations_total{operation=encrypt|decrypt}` and `dds_security_crypto_latency_seconds`.

`forbid(unsafe_code)` is set; HW detection in `aes_gcm_hw.rs` uses `is_x86_feature_detected!` / `is_aarch64_feature_detected!` (no own `unsafe` block).

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (plugin trait), `ring` (AEAD/HMAC/HKDF primitives), optional `zerodds-monitor` (feature `metrics`).
- **Dependents (out):** `zerodds-security-runtime` (plugin lifecycle), `zerodds-security-rtps` (RTPS wrap), `zerodds-dcps` (feature `security`), end-user builds.
- **Feature flags:** `std` (default), `metrics` (default).

### Stability

Public API + wire format RC1-stable. Cross-vendor wire compat with Cyclone/FastDDS applies at the §10.5 wire-bytes level. Major bump on breaking changes.

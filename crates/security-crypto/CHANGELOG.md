# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

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

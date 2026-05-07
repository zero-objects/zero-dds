# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-crypto`-Crate (`CryptographicPlugin`-Impl).

### Spec-Referenzen

- **OMG DDS-Security 1.1** (formal/2018-04-01) §8.5 (CryptographicPlugin-Trait), §9.5 (Builtin-Crypto-Plugin), §10.5 (Wire-Format Tab.73/74/79).
- **OMG DDS-Security 1.2** Delta — KeyMaterial-AES-GCM-GMAC.

### Public-API

- `AesGcmCryptoPlugin::{new, with_suite, with_shared_secret_provider}`.
- `PskCryptoPlugin::{new, with_class_id}`, `CLASS_ID_PSK_CRYPTO`, `HKDF_INFO_PSK_MASTER_KEY`.
- `Suite::{Aes128Gcm, Aes256Gcm, HmacSha256}` + `transform_kind_id`/`from_transform_kind_id`/`key_len`.
- `crypto_transform::{CryptoHeader, CryptoFooter, CryptoTransformIdentifier, CryptoTransformKind, BUILTIN_CRYPTO_PLUGIN, negotiate_transform}`.
- `session_key::{derive_session_key, derive_session_hmac_key, compute_aad, SESSION_KEY_TAG, SESSION_RECEIVER_KEY_TAG, AAD_HEADER_LEN}`.
- `aes_gcm_hw::{Arch, HwCapabilities, report}`.
- `metrics::CryptoOp` (Feature `metrics`).

### Implementierung

`AesGcmCryptoPlugin` haelt einen `RwLock<BTreeMap<CryptoHandle, KeyMaterial>>`. Jeder `KeyMaterial`-Slot traegt Suite + 4-byte transformation_key_id + master_key (16/32 byte) + 32-byte master_salt + 4-byte session_id + AtomicU64-Counter. Encrypt/Decrypt nutzen `derive_session_key(master_key, master_salt, session_id)` als Per-Submessage-AES-Key + `compute_aad(transform_kind, key_id, session_id, extension)` als AES-GCM AAD — Hot-Path ist spec-byte-kompatibel zu Cyclone DDS und FastDDS.

`SharedSecretProvider`-Integration (PKI ↔ Crypto): wenn ein Provider ueber `with_shared_secret_provider` registriert ist, wird `register_matched_remote_*` deterministisch via HKDF-SHA256 aus dem SharedSecret abgeleitet — beide Partner berechnen denselben Master-Key ohne Token-Exchange.

`PskCryptoPlugin` ist ein deterministischer Shared-Secret-Plugin fuer Out-of-Band-Setups (z.B. unattended Embedded). Setup-Token = HKDF(class_id `"DDS:Auth:PSK:1.0"` + setup_salt + identity_hash). Beide Partner derivieren denselben Master-Key.

`CryptoOp` (Feature `metrics`) ist ein RAII-Span — bei `Drop` werden `dds_security_crypto_operations_total{operation=encrypt|decrypt}` und `dds_security_crypto_latency_seconds` aktualisiert.

`forbid(unsafe_code)` ist gesetzt; HW-Detection in `aes_gcm_hw.rs` nutzt `is_x86_feature_detected!` / `is_aarch64_feature_detected!` (kein eigener `unsafe`-Block).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (Plugin-Trait), `ring` (AEAD/HMAC/HKDF/HMAC-Primitives), optional `zerodds-monitor` (Feature `metrics`).
- **Dependents (out):** `zerodds-security-runtime` (Plugin-Lifecycle), `zerodds-security-rtps` (RTPS-Wrap), `zerodds-dcps` (Feature `security`), end-user-Builds.
- **Feature-Flags:** `std` (default), `metrics` (default).

### Stabilitaet

Public-API + Wire-Format RC1-stabil. Cross-Vendor-Wire-Compat zu Cyclone/FastDDS gilt auf §10.5-Wire-Bytes-Ebene. Major-Bump bei Breaking-Changes.

# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- `encode_secured_submessage` and `encode_secured_rtps_message` write the frame
  of DDS-Security 1.2: the SEC_PREFIX and the SRTPS_PREFIX carry the
  CryptoHeader of the key (20 octets: transformation_kind,
  transformation_key_id, session_id, initialization_vector_suffix,
  §7.4.7.6.1, §10.5.2.3), the SEC_BODY the CryptoContent with its length,
  the postfix the CryptoFooter; the RTPS message protection is the form of
  §10.5.3.3.4.6.4 (the header as INFO_SOURCE in the protected body). They
  wrote 16 zero octets (`transformation_kind` 0, NONE) and, for a message, a
  SEC_BODY without length field, a layout §7.4.7 does not have. Only the
  plugin that holds the key can write its CryptoHeader: the functions check
  the frame and hand the transformation to the plugin
  (`encode_data_datawriter_submessage`, `encode_rtps_message_cyclone`,
  `encode_rtps_message_psk` and their decoders), the forms the runtime's
  `SharedSecurityGate` sends. `decode_*` read with the key under `remote`.
  New: `CRYPTO_HEADER_LEN`.

- Removed: `encode_secured_submessage_multi`, `decode_secured_submessage_multi`
  and `MAX_RECEIVER_MACS` (HMAC-SHA256 receiver MACs, no mechanism of the
  specification, no caller in the runtime).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-rtps` crate.

### Spec references

- **OMG DDS-Security 1.1** §7.3.6 (secure submessage), §9.5 (RTPS message protection).

### Public API

- `encode_secured_submessage`, `decode_secured_submessage`.
- `encode_secured_submessage_multi`, `decode_secured_submessage_multi` — multi-receiver MAC list.
- `srtps::{encode_srtps, decode_srtps}` — RTPS header AAD wrap.
- `header_aad::*` — AAD builder.
- Constants `SEC_PREFIX`, `SEC_BODY`, `SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`, `MAX_RECEIVER_MACS`.
- `SecurityRtpsError`.

### Implementation

`encode_secured_submessage` builds the `SEC_PREFIX | SEC_BODY | SEC_POSTFIX` triple via a `&mut dyn CryptographicPlugin` callback. SEC_PREFIX carries the 16-byte transformation identifier (all-zero in the single-plugin path), SEC_BODY contains the ciphertext, SEC_POSTFIX the receiver-specific MAC list (spec §7.3.6.3). Single-receiver paths leave the MAC list empty; `encode_secured_submessage_multi` produces one MAC per receiver for multi-reader setups (max `MAX_RECEIVER_MACS = 64`).

`srtps::encode_srtps` builds the whole-RTPS-message wrap from `SRTPS_PREFIX` + ciphertext body + `SRTPS_POSTFIX` (spec §9.5). The `header_aad` module provides the AAD bind structure for RTPS header authentication.

LE submessage header (`0x01` flag) — all vendors use LE by default; the BE path is major-2.0 additive.

`forbid(unsafe_code)`.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (CryptographicPlugin).
- **Dependents (out):** `zerodds-security-runtime` (wrap/unwrap hooks), `dcps` (feature `security`).
- **Feature flags:** `std` (default).

### Stability

Wire format byte-exact with Cyclone/FastDDS. Public API + wire format RC1-stable.

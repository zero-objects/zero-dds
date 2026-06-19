# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

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

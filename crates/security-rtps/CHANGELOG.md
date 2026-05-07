# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-rtps`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §7.3.6 (Secure Submessage), §9.5 (RTPS-Message Protection).

### Public-API

- `encode_secured_submessage`, `decode_secured_submessage`.
- `encode_secured_submessage_multi`, `decode_secured_submessage_multi` — Multi-Receiver-MAC-Liste.
- `srtps::{encode_srtps, decode_srtps}` — RTPS-Header-AAD-Wrap.
- `header_aad::*` — AAD-Builder.
- Konstanten `SEC_PREFIX`, `SEC_BODY`, `SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`, `MAX_RECEIVER_MACS`.
- `SecurityRtpsError`.

### Implementierung

`encode_secured_submessage` baut `SEC_PREFIX | SEC_BODY | SEC_POSTFIX`-Tripel via `&mut dyn CryptographicPlugin`-Callback. SEC_PREFIX traegt 16-byte Transformation-Identifier (im Single-Plugin-Pfad all-zero), SEC_BODY enthaelt Ciphertext, SEC_POSTFIX Receiver-Specific-MAC-Liste (Spec §7.3.6.3). Single-Receiver-Pfade lassen die MAC-Liste leer; `encode_secured_submessage_multi` produziert MAC-pro-Receiver fuer Multi-Reader-Setups (max `MAX_RECEIVER_MACS = 64`).

`srtps::encode_srtps` baut den ganzen-RTPS-Message-Wrap aus `SRTPS_PREFIX` + ciphertext-body + `SRTPS_POSTFIX` (Spec §9.5). `header_aad`-Modul liefert die AAD-Bind-Struktur fuer RTPS-Header-Authentication.

LE-Submessage-Header (`0x01` flag) — alle Vendoren nutzen LE per Default; BE-Pfad ist Major-2.0-additive.

`forbid(unsafe_code)`.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (CryptographicPlugin).
- **Dependents (out):** `zerodds-security-runtime` (Wrap/Unwrap-Hooks), `dcps` (Feature `security`).
- **Feature-Flags:** `std` (default).

### Stabilitaet

Wire-Format byte-genau zu Cyclone/FastDDS. Public-API + Wire-Format RC1-stabil.

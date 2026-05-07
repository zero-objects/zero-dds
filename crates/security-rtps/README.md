# `zerodds-security-rtps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-rtps/badge.svg)](https://docs.rs/zerodds-security-rtps)

Secure-Submessage-Wrapper + RTPS-Header-AAD-Codec fuer den
[ZeroDDS](https://zerodds.org)-Stack nach OMG DDS-Security 1.1 §7.3.6
+ §9.5. Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §7.3.6 (Secure Submessage), §9.5 (RTPS-Message Protection) |

## Was ist drin

- `encode_secured_submessage` / `decode_secured_submessage`.
- `encode_secured_submessage_multi` / `decode_secured_submessage_multi` (Receiver-Specific-MACs).
- `srtps::{encode_srtps, decode_srtps}`.
- `header_aad`-Modul.
- Konstanten `SEC_PREFIX`, `SEC_BODY`, `SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`, `MAX_RECEIVER_MACS`.

## Stabilitaet

`1.0.0-rc.1`. Wire-Format byte-genau zu Cyclone/FastDDS.

## Tests

```bash
cargo test -p zerodds-security-rtps
```

31 Tests grün.

## Lizenz

Apache-2.0.

# `zerodds-security-rtps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-rtps/badge.svg)](https://docs.rs/zerodds-security-rtps)

Secure submessage wrapper + RTPS header AAD codec for the
[ZeroDDS](https://zerodds.org) stack per OMG DDS-Security 1.1 §7.3.6
+ §9.5. Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §7.3.6 (secure submessage), §9.5 (RTPS message protection) |

## What's inside

- `encode_secured_submessage` / `decode_secured_submessage`.
- `encode_secured_submessage_multi` / `decode_secured_submessage_multi` (receiver-specific MACs).
- `srtps::{encode_srtps, decode_srtps}`.
- `header_aad` module.
- Constants `SEC_PREFIX`, `SEC_BODY`, `SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`, `MAX_RECEIVER_MACS`.

## Stability

`1.0.0-rc.1`. Wire format byte-exact with Cyclone/FastDDS.

## Tests

```bash
cargo test -p zerodds-security-rtps
```

31 tests green.

## License

Apache-2.0.

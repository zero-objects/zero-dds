# `zerodds-security-rtps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-rtps/badge.svg)](https://docs.rs/zerodds-security-rtps)

Secure submessage wrapper + RTPS header AAD codec for the
[ZeroDDS](https://zerodds.org) stack per OMG DDS-Security 1.2 §7.4.6/§7.4.7
+ §9.5. Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.2 | §7.4.6/§7.4.7 (secure submessages), §9.5 (RTPS message protection) |

## What's inside

- `encode_secured_submessage` / `decode_secured_submessage`.
- `srtps::{encode_srtps, decode_srtps}`.
- `header_aad` module.
- Constants `SEC_PREFIX`, `SEC_BODY`, `SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`.

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

## Tests

```bash
cargo test -p zerodds-security-rtps
```

31 tests green.

## License

Apache-2.0.

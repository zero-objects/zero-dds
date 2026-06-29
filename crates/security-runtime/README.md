# `zerodds-security-runtime`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-runtime/badge.svg)](https://docs.rs/zerodds-security-runtime)

Security runtime for the [ZeroDDS](https://zerodds.org) stack:
governance-driven plugin lifecycle, peer-capabilities cache,
built-in data tagging, anti-squatter, heterogeneous-mesh gateway bridge.
Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §8.5.3 (anti-squatter), §9.5 (inbound/outbound) |
| OMG DDS-Security 1.2 | §8.7 (data tagging) |
| ZeroDDS architecture §09 | heterogeneous mesh + delegation |

## What's inside

- `SecurityGate` — high-level adapter governance ↔ crypto ↔ RTPS wrap.
- `engine::GovernancePolicyEngine` — default PolicyEngine.
- `caps::*` + `caps_wire::*` — peer capabilities + SPDP wire codec incl. delegation chain.
- `peer_class::*` — `<peer_class>` match.
- `data_tagging::*` — `DataTaggingPlugin` default impl.
- `builtin_topics::*`, `anti_squatter::*`, `gateway_bridge::*`.

## Layer position

Layer 4. Consumes all 7 security sibling crates.

## Crypto backend selection (`ring` / FIPS `aws-lc` / `wolfCrypt`)

The DDS-Security cryptographic *primitives* (AES-GCM, HMAC-SHA256, HKDF, ECDH,
ECDSA/RSA) are supplied by one compile-time-selected backend. All three are
ring-API-compatible, so the choice is a feature flag — the protocol, the wire
bytes and the public API are identical across them.

| Feature | Backend | Use it for |
|---------|---------|------------|
| `ring-backend` *(default)* | [`ring`](https://crates.io/crates/ring) 0.17 | general use; `no_std`-capable |
| `fips` | [`aws-lc-rs`](https://crates.io/crates/aws-lc-rs) / AWS-LC | a **FIPS-140-3-validatable** module (government / regulated deployments) |
| `wolfcrypt` | wolfSSL / wolfCrypt | FIPS-ready + **DO-178 / MISRA-C / embedded** heritage (avionics, automotive, defense) |

```bash
# Default (ring):
cargo build -p zerodds-security-runtime

# FIPS (aws-lc-rs) — links NO ring at all; the validated module does every op:
cargo build -p zerodds-security-runtime --no-default-features --features std,fips

# wolfCrypt — needs libwolfssl. The published wolfcrypt-sys vendored source is
# incomplete and a system libwolfssl lacks the openssl-compat PBKDF2 symbol the
# shim imports, so point WOLFSSL_SRC at a wolfSSL 5.9.x source tree:
WOLFSSL_SRC=/path/to/wolfssl-5.9.1-stable \
  cargo build -p zerodds-security-runtime --no-default-features --features std,wolfcrypt
```

The same `fips` / `wolfcrypt` features are re-exported by `zerodds-dcps` and
`zerodds-c-api`, so the whole DDS stack — including the C/C++/Python FFI — can be
built with the chosen backend (e.g. `cargo build -p zerodds-c-api --release
--features fips`).

Notes: under `fips` the build links **no** `ring` (the AWS-LC validated module is
the only crypto). Under `wolfcrypt` the DDS-Security primitives are wolfCrypt;
X.509 certificate-chain *verification* still uses the `ring` provider of
`rustls-webpki` (which has no wolfCrypt provider upstream) — the only place
`ring` is linked in a wolfCrypt build. Exactly one backend feature must be
active; selecting `fips` or `wolfcrypt` overrides the default `ring-backend`.

## Stability

`1.0.0-rc.1`.

## Tests

```bash
cargo test -p zerodds-security-runtime
```

214+ tests green.

## License

Apache-2.0.

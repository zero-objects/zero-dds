# `zerodds-security-keyexchange`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-keyexchange/badge.svg)](https://docs.rs/zerodds-security-keyexchange)

Ephemeral Diffie-Hellman key agreement for the DDS-Security
authentication handshake per OMG DDS-Security 1.1 §8.3.2. Wrapper around
`ring::agreement` + `ring::hkdf`. Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §8.3.2 (authentication handshake), §8.3.2.11 (key establishment) |

## What's inside

- **`KeyExchange`** + **`Suite::X25519`** / **`Suite::P256Ecdh`** — ephemeral-DH roundtrip with `derive_shared_secret()` → 32-byte HKDF-SHA256 output.

## Layer position

Layer 4. Consumed by [`zerodds-security-pki`](../security-pki) in the authentication-handshake state machine.

## Quickstart

```rust
use zerodds_security_keyexchange::KeyExchange;

let alice = KeyExchange::new().expect("alice");
let bob = KeyExchange::new().expect("bob");

let a_pub = alice.public_key().to_vec();
let b_pub = bob.public_key().to_vec();

let s1 = alice.derive_shared_secret(&b_pub).expect("alice derive");
let s2 = bob.derive_shared_secret(&a_pub).expect("bob derive");
assert_eq!(s1, s2);
```

## Suite coverage

| Suite | Use case |
|-------|----------|
| `X25519` (default) | Modern, 32-byte public key |
| `P256Ecdh` | Classic ECDH alternative |

## Non-goals

RSA-OAEP key transport (spec §8.3.2.11 alternative form) is not in RC1 — all relevant vendors (Cyclone DDS, FastDDS, RTI Connext) speak ECDH/X25519.

## Stability

`1.0.0-rc.1`. Public API + wire format (public-key encoding) RC1-stable.

## Tests

```bash
cargo test -p zerodds-security-keyexchange
```

11 unit tests + 1 doc test green.

## License

Apache-2.0.

## See also

- `docs/spec-coverage/dds-security-1.2.md`.
- [`zerodds-security-pki`](../security-pki).
- [`zerodds-security-crypto`](../security-crypto).

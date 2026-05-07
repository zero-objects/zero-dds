# `zerodds-security-keyexchange`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-keyexchange/badge.svg)](https://docs.rs/zerodds-security-keyexchange)

Ephemeral-Diffie-Hellman Key-Agreement fuer den DDS-Security
Authentication-Handshake nach OMG DDS-Security 1.1 §8.3.2. Wrapper um
`ring::agreement` + `ring::hkdf`. Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §8.3.2 (Authentication-Handshake), §8.3.2.11 (Key-Establishment) |

## Was ist drin

- **`KeyExchange`** + **`Suite::X25519`** / **`Suite::P256Ecdh`** — Ephemeral-DH-Roundtrip mit `derive_shared_secret()` → 32-byte HKDF-SHA256-Output.

## Schichten-Position

Layer 4. Konsumiert von [`zerodds-security-pki`](../security-pki) im Authentication-Handshake-State-Machine.

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

## Suite-Coverage

| Suite | Use-Case |
|-------|----------|
| `X25519` (Default) | Modern, 32-byte Public-Key |
| `P256Ecdh` | Klassische ECDH-Alternative |

## Nicht-Ziele

RSA-OAEP-Key-Transport (Spec §8.3.2.11 alternative Form) ist nicht in RC1 — alle relevanten Vendoren (Cyclone DDS, FastDDS, RTI Connext) sprechen ECDH/X25519.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Wire-Format (Public-Key-Encoding) RC1-stabil.

## Tests

```bash
cargo test -p zerodds-security-keyexchange
```

11 Unit-Tests + 1 Doc-Test grün.

## Lizenz

Apache-2.0.

## Siehe auch

- `docs/spec-coverage/dds-security-1.2.md`.
- [`zerodds-security-pki`](../security-pki).
- [`zerodds-security-crypto`](../security-crypto).

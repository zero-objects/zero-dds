# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `KxSuite::DhModp2048_256`: `DHE+MODP-2048-256` (DDS-Security 1.2 §8.3
  Table 26, Id 0x20), finite-field Diffie-Hellman over the RFC 5114 §2.3 group
  with `crypto-bigint` (Montgomery form; `pow` takes the same time for every
  256-bit exponent, the exponent is zeroed on drop); the peer's key is checked
  as SP 800-56A §5.6.2.3.1 asks. The constants are compared with the text of
  the RFC by a test (`tests/fixtures/rfc5114_2_3.txt`).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-keyexchange` crate.

### Spec references

- **OMG DDS-Security 1.1** §8.3.2 (authentication handshake) + §8.3.2.11 (key establishment).

### Public API

- `KeyExchange::{new, with_suite, public_key, derive_shared_secret}`.
- `Suite::{X25519, P256Ecdh}`.

### Implementation

`KeyExchange::new` generates an ephemeral key pair via `ring::agreement::EphemeralPrivateKey` (X25519 or P-256 ECDH). `derive_shared_secret(&remote_pub)` calls `ring::agreement::agree_ephemeral` and expands the result via HKDF-SHA256 to 32 bytes. Both sides of the DH operation compute the same SharedSecret deterministically.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (plugin trait + errors), `ring` (crypto primitives).
- **Dependents (out):** `zerodds-security-pki` (handshake state machine).
- **Feature flags:** `std` (default).

### Stability

Public API + wire format (public-key encoding) RC1-stable.

### Removed (RsaKeyWrap)

Before cleanup there was an `rsa_wrap` module with an `RsaKeyWrap` struct. The `wrap_secret` implementation was explicitly a placeholder ("ring 0.17 exposes no RSA encrypt API; currently the function returns the input with a 16-byte random mask prepended, so integration tests can validate the call path"). That was a phantom API without spec compliance — dropped for RC1, because:
1. 0 external production refs (only own tests).
2. X25519 + P-256 ECDH cover all modern vendors.
3. RSA-OAEP key transport (spec §8.3.2.11) is an optional alternative.

If a concrete legacy use case appears, the path via the `rsa` crate is reintroduced as a major-2.0 additive extension.

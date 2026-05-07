# `zerodds-corba-csiv2`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-csiv2/badge.svg)](https://docs.rs/zerodds-corba-csiv2)

OMG CORBA 3.3 Part 3 — Common Secure Interoperability v2 (CSIv2)
§24 voller Stack: Association-Options, Compound-Sec-Mech-List,
GSSUP-Token, SAS-Protocol, TLS-Mechanism-OID. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 3 | §24.2 (SAS-Protocol), §24.2.4 (Association-Options), §24.2.6.5 (Compound-Sec-Mech-List + TLS-Mechanism-OID), §24.7 (GSSUP) |

## Was ist drin

- **`AssociationOptions`** — §24.2.4 Bitmask (`Integrity` /
  `Confidentiality` / `EstablishTrustInTarget` /
  `EstablishTrustInClient` / `IdentityAssertion` /
  `DelegationByClient` / `NoProtection`).
- **`CompoundSecMech` / `CompoundSecMechList` / `AsContextSec` /
  `SasContextSec`** — §24.2.6.5 `TAG_CSI_SEC_MECH_LIST`-Component-
  Body.
- **`GssupCredentialToken` / `INITIAL_CONTEXT_TOKEN_TAG`** — §24.7
  Username/Password-Token mit `INITIAL_CONTEXT_TOKEN`-Wrapping.
- **`SasMessage` / `EstablishContext` / `CompleteEstablishContext` /
  `MessageInContext` / `ContextError`** — §24.2 SAS-Protocol-
  Messages.
- **`IdentityToken`** — §24.2.5 Identity-Token-Form.

## Schichten-Position

Layer 8 — CORBA-Stack (Tier-A). Sitzt auf `zerodds-cdr` (Wire-
Codec). Konsumenten sind GIOP-/IIOP-Server (Layer-8-Tier-B/C) mit
Security-Stack-Konfiguration.

## Quickstart

```rust
use zerodds_corba_csiv2::AssociationOptions;

let opts = AssociationOptions(AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY);
assert!(opts.0 & AssociationOptions::INTEGRITY != 0);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Bitmasks + SAS-Protocol-Wire-Format sind
RC1-stabil; durch OMG-Spec fixiert.

## Tests

```bash
cargo test -p zerodds-corba-csiv2
```

15 Unit-Tests grün.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/corba-csiv2.md`](../../docs/release/rc1-reviews/corba-csiv2.md) — RC1-Review.

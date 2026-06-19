# `zerodds-corba-csiv2`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-csiv2/badge.svg)](https://docs.rs/zerodds-corba-csiv2)

OMG CORBA 3.3 Part 3 — Common Secure Interoperability v2 (CSIv2)
§24 full stack: association options, compound sec-mech list,
GSSUP token, SAS protocol, TLS mechanism OID. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 3 | §24.2 (SAS protocol), §24.2.4 (association options), §24.2.6.5 (compound sec-mech list + TLS mechanism OID), §24.7 (GSSUP) |

## What is inside

- **`AssociationOptions`** — §24.2.4 bitmask (`Integrity` /
  `Confidentiality` / `EstablishTrustInTarget` /
  `EstablishTrustInClient` / `IdentityAssertion` /
  `DelegationByClient` / `NoProtection`).
- **`CompoundSecMech` / `CompoundSecMechList` / `AsContextSec` /
  `SasContextSec`** — §24.2.6.5 `TAG_CSI_SEC_MECH_LIST` component
  body.
- **`GssupCredentialToken` / `INITIAL_CONTEXT_TOKEN_TAG`** — §24.7
  username/password token with `INITIAL_CONTEXT_TOKEN` wrapping.
- **`SasMessage` / `EstablishContext` / `CompleteEstablishContext` /
  `MessageInContext` / `ContextError`** — §24.2 SAS protocol
  messages.
- **`IdentityToken`** — §24.2.5 identity-token form.

## Layer position

Layer 8 — CORBA stack (Tier A). Sits on `zerodds-cdr` (wire codec).
Consumers are GIOP/IIOP servers (Layer 8, Tier B/C) with
security-stack configuration.

## Quickstart

```rust
use zerodds_corba_csiv2::AssociationOptions;

let opts = AssociationOptions(AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY);
assert!(opts.0 & AssociationOptions::INTEGRITY != 0);
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. The public API, bitmasks, and SAS protocol wire format
are RC1-stable; fixed by the OMG spec.

## Tests

```bash
cargo test -p zerodds-corba-csiv2
```

15 unit tests passing.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/corba-csiv2.md`](../../docs/release/rc1-reviews/corba-csiv2.md) — RC1 review.

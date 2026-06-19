# `zerodds-ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-ccm/badge.svg)](https://docs.rs/zerodds-ccm)

OMG CCM 4.0 (`formal/06-04-01`) — CORBA Component Model.
Equivalent-IDL transformation for Component / Home / EventType,
Components::* core types, Lightweight CCM Profile (§13).
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CCM 4.0 | §6.3.2 (Component Implied-IDL) |
| OMG CCM 4.0 | §6.4.1 (Home Implied-IDL + PrimaryKey) |
| OMG CCM 4.0 | §6.5.1 (Receptacles + Cookie) |
| OMG CCM 4.0 | §6.6.x (Events / Emitters / Publishers / Subscribers) |
| OMG CCM 4.0 | §6.7.1 (EventType Equivalent-IDL) |
| OMG CCM 4.0 | §13 (Lightweight CCM Profile) |
| OMG DDS4CCM 1.1 | §6 (DDS4CCM-Connector-Patterns) |

## What's inside

- **`model`** — `Cookie`, `PortDescription`, `FacetDescription`,
  `ReceptacleDescription`, `ConsumerDescription`, `EmitterDescription`,
  `SubscriberDescription`, `PublisherDescription`,
  `ConnectionDescription`, `ConfigValue` as Components::* valuetypes.
- **`transform`** — `transform_component` / `transform_home` /
  `transform_event_type` produce the spec-conformant implied-IDL
  definitions from `zerodds_idl::ast` inputs.
- **`lightweight`** — `filter_to_lightweight(spec)` reduces a full-CCM
  AST to the LwCCM subset (Spec §13).
- **`validate`** — `validate_primary_key`, `apply_factory_finder_body`,
  `InitOp` for the Spec §6.4.1 constraints.
- **`dds4ccm`** — DDS4CCM-specific Equivalent-IDL extensions.

## What's not covered

CCM 4.0 §7 (CIDL), §8 (Implementation Framework), §9 (Container),
§10 (EJB integration), §11 (IFR Metamodel), §12 (CIF Metamodel), §14
(Deployment PSM), §15 (Deployment IDL), §16 (XSD) are explicitly `n/a`
in this crate, because they require a CORBA-ORB + CCM-container runtime
that ZeroDDS deliberately does not host itself (see
`crates/corba-ccm-ejb/` for an EJB bridge at the model level).

## Example

```rust
use zerodds_ccm::Cookie;

let c = Cookie::new(vec![0x01, 0x02, 0x03]);
assert_eq!(c.cookie_value, vec![0x01, 0x02, 0x03]);
// Spec §6.5.2.4: Truncation auf Base behaelt cookieValue.
assert_eq!(c.truncate_to_base().cookie_value, c.cookie_value);
```

## Tests

```bash
cargo test -p zerodds-ccm
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [Components](../../documentation/02-architecture/components.md)
- [Spec-Coverage Audit](../../docs/spec-coverage/omg-ccm-4.0.md)
- [DDS4CCM Audit](../../docs/spec-coverage/dds4ccm-1.1.md)

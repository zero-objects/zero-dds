# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-ccm` crate.

### Spec-Referenzen

- **OMG CCM 4.0** (`formal/06-04-01`): §6 Component Model
  (Component/Home/EventType-Equivalent-IDL), §6.3.2 (Component
  Implied-IDL), §6.4.1 (Home Implied-IDL), §6.5.1 (Receptacles),
  §6.6.x (Events / Emitters / Publishers), §6.7.1 (EventType
  Equivalent-IDL), §13 Lightweight CCM Profile.
- **OMG DDS4CCM 1.1** (`formal/2012-02-01`): §6 connector patterns
  (DDS4CCM-specific subset layered above the CCM model).

### Public-API

**`model` module (Components::* Core):**
- `Cookie { cookie_value }` — receptacle identifier (Spec §6.5.2.4).
- `FeatureName`, `RepositoryId`, `FailureReason` — spec type aliases.
- `PortDescription`, `FacetDescription`, `ReceptacleDescription`,
  `ConsumerDescription`, `EmitterDescription`, `PublisherDescription`,
  `SubscriberDescription`, `ConnectionDescription`, `ConfigValue` —
  Components::*-Valuetypes (Spec §6.4.3.3 / §6.5.3 / §6.6.x).

**`transform` module (Equivalent-IDL):**
- `transform_component(comp) -> ComponentEquivalent` — Spec §6.3.2.
- `transform_home(home) -> HomeEquivalent` — Spec §6.4.1.
- `transform_event_type(evt) -> EventTypeEquivalent` — Spec §6.7.1.
- `scoped_name(...)` — helper for Equivalent-IDL naming.

**`lightweight` module (LwCCM Profile §13):**
- `filter_to_lightweight(spec) -> Result<Spec, LightweightFilterError>`
  — reduces a full-CCM Equivalent-IDL AST to the LwCCM subset
  (no persistence, no CCMHome with PrimaryKey, etc.).

**`validate` module:**
- `validate_primary_key(key)` — Spec §6.4.1.6 PrimaryKey constraints.
- `apply_factory_finder_body(home_def, body)` — Spec §6.4.1.4
  factory/finder operations.
- `InitOp`, `PrimaryKeyError`.

**`dds4ccm` module:**
- DDS4CCM-specific Equivalent-IDL extensions (connectors).

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` (default feature `std`
pulls in `alloc`); `#![forbid(unsafe_code)]`.

One workspace dependency: `zerodds-idl` (AST layer for Component / Home /
EventType definitions). Input is
`zerodds_idl::ast::{ComponentDef, HomeDef, EventDef}`; output is
spec-conformant `interface` / `valuetype` definitions that an IDL
compiler "implicitly defines" per Spec §6.3.2 / §6.4.1 / §6.5.1 /
§6.6.x / §6.7.1.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** `zerodds-idl`.
- **Dependents (out):** `zerodds-corba-ccm` (CORBA-CCM wrapper),
  `zerodds-ami4ccm` (related connector model), codegen consumers
  via Equivalent-IDL.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Equivalent-IDL output form: fixed by OMG spec §6.x.
- LwCCM filter subset: fixed by OMG spec §13.

### Spec coverage limitations

CCM 4.0 §7 (CIDL), §8 (Implementation Framework), §9 (Container),
§10 (EJB integration), §11 (IFR Metamodel), §12 (CIF Metamodel), §14
(Deployment PSM), §15 (Deployment IDL), §16 (XSD) are explicitly `n/a`
in this crate, because they require a CORBA-ORB + CCM-container runtime
that ZeroDDS deliberately does not host itself. Rationale in the audit
file `docs/spec-coverage/omg-ccm-4.0.md`.

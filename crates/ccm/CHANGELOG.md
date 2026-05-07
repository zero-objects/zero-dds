# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-ccm`-Crate.

### Spec-Referenzen

- **OMG CCM 4.0** (`formal/06-04-01`): §6 Component Model
  (Component/Home/EventType-Equivalent-IDL), §6.3.2 (Component
  Implied-IDL), §6.4.1 (Home Implied-IDL), §6.5.1 (Receptacles),
  §6.6.x (Events / Emitters / Publishers), §6.7.1 (EventType
  Equivalent-IDL), §13 Lightweight CCM Profile.
- **OMG DDS4CCM 1.1** (`formal/2012-02-01`): §6 Connector-Patterns
  (DDS4CCM-spezifischer Subset oberhalb des CCM-Models).

### Public-API

**`model`-Modul (Components::* Core):**
- `Cookie { cookie_value }` — Receptacle-Identifier (Spec §6.5.2.4).
- `FeatureName`, `RepositoryId`, `FailureReason` — Spec-Type-Aliases.
- `PortDescription`, `FacetDescription`, `ReceptacleDescription`,
  `ConsumerDescription`, `EmitterDescription`, `PublisherDescription`,
  `SubscriberDescription`, `ConnectionDescription`, `ConfigValue` —
  Components::*-Valuetypes (Spec §6.4.3.3 / §6.5.3 / §6.6.x).

**`transform`-Modul (Equivalent-IDL):**
- `transform_component(comp) -> ComponentEquivalent` — Spec §6.3.2.
- `transform_home(home) -> HomeEquivalent` — Spec §6.4.1.
- `transform_event_type(evt) -> EventTypeEquivalent` — Spec §6.7.1.
- `scoped_name(...)` — Helper fuer Equivalent-IDL-Naming.

**`lightweight`-Modul (LwCCM Profile §13):**
- `filter_to_lightweight(spec) -> Result<Spec, LightweightFilterError>`
  — reduziert ein voll-CCM-Equivalent-IDL-AST auf das LwCCM-Subset
  (kein Persistence, kein CCMHome-mit-PrimaryKey, etc.).

**`validate`-Modul:**
- `validate_primary_key(key)` — Spec §6.4.1.6 PrimaryKey-Constraints.
- `apply_factory_finder_body(home_def, body)` — Spec §6.4.1.4
  Factory/Finder-Operations.
- `InitOp`, `PrimaryKeyError`.

**`dds4ccm`-Modul:**
- DDS4CCM-spezifische Equivalent-IDL-Erweiterungen (Connectors).

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` (default-feature `std`
zieht `alloc` rein); `#![forbid(unsafe_code)]`.

Eine Workspace-Dep: `zerodds-idl` (AST-Layer fuer Component-/Home-/
EventType-Definitionen). Eingabe ist
`zerodds_idl::ast::{ComponentDef, HomeDef, EventDef}`; Ausgabe sind
spec-konforme `interface`-/`valuetype`-Definitionen, die ein IDL-
Compiler "implicitly defines" laut Spec §6.3.2 / §6.4.1 / §6.5.1 /
§6.6.x / §6.7.1.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-idl`.
- **Dependents (out):** `zerodds-corba-ccm` (CORBA-CCM-Wrapper),
  `zerodds-ami4ccm` (verwandtes Connector-Modell), Codegen-Konsumenten
  via Equivalent-IDL.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Equivalent-IDL-Output-Form: durch OMG-Spec §6.x fixiert.
- LwCCM-Filter-Subset: durch OMG-Spec §13 fixiert.

### Spec-Coverage-Begrenzungen

CCM 4.0 §7 (CIDL), §8 (Implementation Framework), §9 (Container),
§10 (EJB-Integration), §11 (IFR-Metamodel), §12 (CIF-Metamodel), §14
(Deployment-PSM), §15 (Deployment-IDL), §16 (XSD) sind explizit `n/a`
in dieser Crate, weil sie eine CORBA-ORB- + CCM-Container-Runtime
verlangen, die ZeroDDS bewusst nicht selbst hostet. Begruendung im
Audit-File `docs/spec-coverage/omg-ccm-4.0.md`.

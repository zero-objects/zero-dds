# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-dnc` crate.

### Spec references

- **OMG Deployment & Configuration 4.0** (`formal/2006-04-02`):
  §6 (Domain-Level Data Models), §7 (Component Data),
  §8 (RepositoryManager), §9 (ExecutionManager / NodeManager /
  DomainApplicationManager / NodeApplicationManager),
  §10 (XML encoding of the plan files).

### Public API

- `plan::{DeploymentPlan, ComponentPackageDescription,
  ImplementationDescription, ImplementationDependency,
  InstanceDeploymentDescription, PackageConfiguration,
  PackagedComponentImplementation, PlanError}` — data model for
  DPD/CPD/IDD/PSD/PSD (D&C §6 + §7).
- `xml::{ParseError, parse_plan_xml}` — XML loader per §10
  XML encoding.
- `repository::RepositoryManager` — RepositoryManager (§8).
- `execution::{ExecutionManager, DomainApplication,
  DomainApplicationManager}` — execution layer (§9).
- `node::{NodeManager, NodeApplication, NodeApplicationManager}` —
  node layer (§9).
- `container_host::{ContainerHost, HostError}` — bridge between the
  D&C plan application and `corba-ccm::Container`.

### Implementation

`#![no_std]` with `extern crate alloc`; `#![forbid(unsafe_code)]`.
The XML loader is fully spec-compliant for §10 encoding with attribute
and element mapping.

`ContainerHost` bridges the gap: a plan-application run instantiates a
`Container` via the repository implementations and maps
instances→components through the CCM lifecycle.

### Architecture

- **Layer:** 8 (CORBA stack, Tier B).
- **Dependencies (in):** `zerodds-corba-ccm`.
- **Dependents (out):** hosting applications (caller layer for the
  plan bootstrap).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- The plan data model strictly follows the §6/§7 schema.
- §10 XML roundtrip-tested.

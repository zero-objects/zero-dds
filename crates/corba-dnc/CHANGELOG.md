# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-dnc`-Crate.

### Spec-Referenzen

- **OMG Deployment & Configuration 4.0** (`formal/2006-04-02`):
  §6 (Domain-Level Data Models), §7 (Component-Data),
  §8 (RepositoryManager), §9 (ExecutionManager / NodeManager /
  DomainApplicationManager / NodeApplicationManager),
  §10 (XML-Encoding der Plan-Files).

### Public-API

- `plan::{DeploymentPlan, ComponentPackageDescription,
  ImplementationDescription, ImplementationDependency,
  InstanceDeploymentDescription, PackageConfiguration,
  PackagedComponentImplementation, PlanError}` — Datenmodell fuer
  DPD/CPD/IDD/PSD/PSD (D&C §6 + §7).
- `xml::{ParseError, parse_plan_xml}` — XML-Loader gemaess §10
  XML-Encoding.
- `repository::RepositoryManager` — RepositoryManager (§8).
- `execution::{ExecutionManager, DomainApplication,
  DomainApplicationManager}` — Execution-Layer (§9).
- `node::{NodeManager, NodeApplication, NodeApplicationManager}` —
  Node-Layer (§9).
- `container_host::{ContainerHost, HostError}` — Bridge zwischen
  D&C-Plan-Application und `corba-ccm::Container`.

### Implementierung

`#![no_std]` mit `extern crate alloc`; `#![forbid(unsafe_code)]`.
XML-Loader voll spec-konform fuer §10-Encoding mit Attribute- und
Element-Mapping.

`ContainerHost` schlaegt die Bruecke: ein Plan-Application-Run
instanziiert via Repository-Implementations einen `Container` und
mappt Instances→Components mittels CCM-Lifecycle.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-B).
- **Dependencies (in):** `zerodds-corba-ccm`.
- **Dependents (out):** Hosting-Anwendungen (Caller-Layer fuer
  Plan-Bootstrap).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Plan-Datenmodell folgt strikt §6/§7-Schema.
- §10-XML-Roundtrip-getestet.

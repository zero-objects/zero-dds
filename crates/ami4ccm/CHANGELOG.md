# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-ami4ccm`-Crate.

### Spec-Referenzen

- **OMG AMI4CCM 1.1** (`formal/2015-08-03`): §7.3 (Implied-IDL fuer
  AMI4CCM-Interface), §7.5 (Implied-IDL fuer ReplyHandler), §7.4
  (ExceptionHolder-Datenmodell), §7.7 (Pragmas: `ami4ccm interface`
  und `ami4ccm receptacle`).

### Public-API

**`pragma`-Modul:**
- `Ami4CcmPragma::{Interface { name }, Receptacle { name }}` — geparste
  Pragma-Variante (Spec §7.7).
- `parse_pragma(line) -> Result<Ami4CcmPragma, ParsePragmaError>` —
  Source-Line-Parser inkl. Whitespace-Toleranz.
- `ParsePragmaError::{NotAmi4ccmPragma, UnknownTag, MalformedQuotedName, EmptyName}`.

**`transform`-Modul:**
- `transform_interface(iface) -> Ami4CcmInterfaces` — erzeugt aus einem
  `zerodds_idl::ast::InterfaceDef` die zwei abgeleiteten Local-
  Interfaces `AMI4CCM_<Iface>` + `AMI4CCM_<Iface>ReplyHandler` (Spec
  §7.3 + §7.5).
- `transform_interface_in_context(iface, ctx)` — Variante mit
  Scope-Resolver-Kontext fuer Cross-Module-Type-Resolution.
- `Ami4CcmInterfaces { async_iface, reply_handler }`,
  `TransformContext`.

**`exception_holder`-Modul:**
- `ExceptionHolder` — Datenmodell fuer Spec §7.4.1 Exception-Lieferung.
- `UserExceptionBase`-Trait fuer ExceptionHolder-Carry.

**`pragma`/`scope_resolver`/`transform`-Synergie:**
- `populate_from_specification(spec) -> ScopeContext` — sammelt alle
  Pragma-Eintraege auf Specification-Level und liefert das
  Scope-Resolver-Kontext.
- `context_from_specification(spec)` — gleicher Pfad fuer Single-Spec-
  Aufrufe.

**`connector`/`deployment`/`multiplex`-Module:**
- `Connector`, `ConnectorPort`, `Facet`, `PortType` — Connector-Modell
  (Spec §7.6).
- `ConnectorImplementation`, `ConnectorPlanFragment`,
  `ImplementationDescriptor`, `PlanInstance` — D&C-Plan-Fragment-
  Modelle.
- `ReceptacleArity::{Simplex, Multiplex}` + Helpers
  `context_method_for_receptacle` und `sequence_typedef_for_interface`
  fuer Multi-Receptacle-Codegen.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` (default-feature `std`
zieht `alloc` rein); `#![forbid(unsafe_code)]`. Eine Workspace-Dep:
`zerodds-idl` (AST-Layer fuer Interface/Module-Definitionen).

Die Transformation arbeitet auf dem AST-Layer von `zerodds_idl::ast`:
Eingabe ist `InterfaceDef`, Ausgabe sind zwei neu konstruierte
`InterfaceDef`-Instanzen mit `InterfaceKind::Local`, die jedes Codegen-
Backend (cpp/cs/java/rust/ts) wie normale Interfaces behandeln kann.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-idl`.
- **Dependents (out):** keine produktiven extern (Connector-Fragment ist
  CCM-Container-Konsumenten-Sache; siehe `corba-ccm` + `corba-ccm-lib`).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- AST-Eingabe-Form: gekoppelt an `zerodds-idl` AST-Stabilitaet.
- Implied-IDL-Output-Form: durch OMG-Spec §7.3/§7.5 fixiert.

### Conformance-Punkte

- **Conformance-Punkt 1 (Implied-IDL-Transformation):** voll abgedeckt
  (alle drei abgeleiteten Operations-Familien `sendc_*`, `*_excep`,
  ReplyHandler-Callbacks).
- **Conformance-Punkt 2 (Connector-Fragment):** Modell-Layer (`connector`,
  `deployment`) ist abgedeckt; das Connector-Runtime-Hosting ist
  CCM-Container-Konsumenten-Sache. Siehe Audit-File
  `docs/spec-coverage/omg-ami4ccm-1.1.md`.

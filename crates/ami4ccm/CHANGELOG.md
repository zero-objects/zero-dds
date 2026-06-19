# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-ami4ccm` crate.

### Spec references

- **OMG AMI4CCM 1.1** (`formal/2015-08-03`): §7.3 (implied IDL for the
  AMI4CCM interface), §7.5 (implied IDL for the ReplyHandler), §7.4
  (ExceptionHolder data model), §7.7 (pragmas: `ami4ccm interface`
  and `ami4ccm receptacle`).

### Public API

**`pragma` module:**
- `Ami4CcmPragma::{Interface { name }, Receptacle { name }}` — the parsed
  pragma variant (spec §7.7).
- `parse_pragma(line) -> Result<Ami4CcmPragma, ParsePragmaError>` —
  source-line parser including whitespace tolerance.
- `ParsePragmaError::{NotAmi4ccmPragma, UnknownTag, MalformedQuotedName, EmptyName}`.

**`transform` module:**
- `transform_interface(iface) -> Ami4CcmInterfaces` — derives, from a
  `zerodds_idl::ast::InterfaceDef`, the two derived local interfaces
  `AMI4CCM_<Iface>` + `AMI4CCM_<Iface>ReplyHandler` (spec
  §7.3 + §7.5).
- `transform_interface_in_context(iface, ctx)` — variant with
  scope-resolver context for cross-module type resolution.
- `Ami4CcmInterfaces { async_iface, reply_handler }`,
  `TransformContext`.

**`exception_holder` module:**
- `ExceptionHolder` — data model for spec §7.4.1 exception delivery.
- `UserExceptionBase` trait for ExceptionHolder carry.

**`pragma`/`scope_resolver`/`transform` synergy:**
- `populate_from_specification(spec) -> ScopeContext` — collects all
  pragma entries at the specification level and returns the
  scope-resolver context.
- `context_from_specification(spec)` — the same path for single-spec
  calls.

**`connector`/`deployment`/`multiplex` modules:**
- `Connector`, `ConnectorPort`, `Facet`, `PortType` — connector model
  (spec §7.6).
- `ConnectorImplementation`, `ConnectorPlanFragment`,
  `ImplementationDescriptor`, `PlanInstance` — D&C plan-fragment
  models.
- `ReceptacleArity::{Simplex, Multiplex}` + helpers
  `context_method_for_receptacle` and `sequence_typedef_for_interface`
  for multi-receptacle codegen.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` (default feature `std`
pulls in `alloc`); `#![forbid(unsafe_code)]`. One workspace dep:
`zerodds-idl` (AST layer for interface/module definitions).

The transformation operates on the AST layer of `zerodds_idl::ast`:
the input is `InterfaceDef`, the output is two newly constructed
`InterfaceDef` instances with `InterfaceKind::Local`, which every codegen
backend (cpp/cs/java/rust/ts) can treat like normal interfaces.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** `zerodds-idl`.
- **Dependents (out):** none in production externally (the connector fragment is
  a concern of CCM-container consumers; see `corba-ccm` + `corba-ccm-lib`).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- AST input form: coupled to `zerodds-idl` AST stability.
- Implied-IDL output form: fixed by OMG spec §7.3/§7.5.

### Conformance points

- **Conformance point 1 (implied-IDL transformation):** fully covered
  (all three derived operation families `sendc_*`, `*_excep`,
  ReplyHandler callbacks).
- **Conformance point 2 (connector fragment):** the model layer (`connector`,
  `deployment`) is covered; connector runtime hosting is a
  concern of CCM-container consumers. See audit file
  `docs/spec-coverage/omg-ami4ccm-1.1.md`.

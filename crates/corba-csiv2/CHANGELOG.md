# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- `SASContextBody` is a union over `typedef short MsgType` (`CSI.idl`):
  the discriminator was written and read as an `unsigned long`, four octets
  at offset 4 instead of two at offset 2. ZeroDDS read its own messages
  back because the padding in front is zero; JacORB 3.9 read every
  `CompleteEstablishContext` and `ContextError` from the bridge as
  `MTEstablishContext` and failed the call ("Could not parse SAS reply").
- `MTMessageInContext` is 5, not 2.
- `AuthorizationToken` is `sequence<AuthorizationElement>`
  (`AuthorizationElement { the_type, the_element }`), not
  `sequence<octet>`; a token with elements was misread.
- `IdentityToken` has a `default` branch (`IdentityExtension`); an
  unknown `IdentityTokenType` decodes into `IdentityToken::Extension`
  instead of failing the whole context, and the authenticator refuses it.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-csiv2` crate.

### Spec references

- **OMG CORBA 3.3 Part 2** §10 (Secure Interoperability / CSIv2): §10.2 (Protocol Message Definitions), §10.3 (Security Attribute Service / SAS), §10.4 (Transport Security Mechanisms), §10.5 (IOR with security tags), §10.6 (Conformance Levels), §10.9 (IDL for CSIv2). Spec coverage in `docs/spec-coverage/corba-3.3.md` §10 all set to `done`.
- (Note: an older doc location mentions §24 — that is a counting variant in the spec; the current spec-coverage doc follows the Part 2 §10 numbering.)

### Public API

- `AssociationOptions(u16)` with constants `NO_PROTECTION`, `INTEGRITY`, `CONFIDENTIALITY`, `DETECT_REPLAY`, `DETECT_MISORDERING`, `ESTABLISH_TRUST_IN_TARGET`, `ESTABLISH_TRUST_IN_CLIENT`, `NO_DELEGATION`, `SIMPLE_DELEGATION`, `COMPOSITE_DELEGATION`, `IDENTITY_ASSERTION`, `NO_PROTECTION_LIBARY`.
- `CompoundSecMech`, `CompoundSecMechList`, `AsContextSec`, `SasContextSec`.
- `GssupCredentialToken`, `INITIAL_CONTEXT_TOKEN_TAG`.
- `SasMessage` + `EstablishContext`, `CompleteEstablishContext`, `MessageInContext`, `ContextError`, `IdentityToken`.

### Implementation

**Algorithms + data structures:** `AssociationOptions` is a `pub struct AssociationOptions(pub u16)` with the bitmask from Spec §10.6 as `pub const` constants (the caller bit-ORs / -ANDs directly on the `u16`). `CompoundSecMechList` is embedded in the IOR as a `TAG_CSI_SEC_MECH_LIST` TaggedComponent; CDR encoding via `zerodds-cdr`. `SasMessage` is a variant enum with the four Spec §10.2 types (EstablishContext / CompleteEstablishContext / MessageInContext / ContextError); each variant has its own wire representation. `GssupCredentialToken` is wrapped with the `INITIAL_CONTEXT_TOKEN` tag (Spec §10.9 Annex / GSSUP). `IdentityToken` covers the §10.3 identity-token form.

**Performance / security / no_std:** Wire conformance: octet-by-octet per the Spec §10 CDR format; all inline tests are roundtrip tests against the spec bit patterns. `#![cfg_attr(not(feature = "std"), no_std)]` + `#![forbid(unsafe_code)]` + `extern crate alloc`. Workspace lints `unsafe_code = forbid`, `unwrap_used`/`expect_used`/`panic` = `warn`; exempted in test modules via `#[allow]`. Cross-vendor interop path: the wire-bytes level is verified through a CDR roundtrip (a live test against TAO/JacORB is the daemon caller's responsibility).

### Architecture

- **Layer:** 8 (CORBA stack, Tier A).
- **Dependencies (in):** `zerodds-cdr` (wire codec).
- **Dependents (out):** GIOP/IIOP servers (Layer 8, Tier B/C) with security-stack configuration.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by OMG CORBA 3.3 Part 3 §24.

# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-csiv2`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 2** §10 (Secure Interoperability / CSIv2): §10.2 (Protocol Message Definitions), §10.3 (Security Attribute Service / SAS), §10.4 (Transport Security Mechanisms), §10.5 (IOR mit Security-Tags), §10.6 (Conformance Levels), §10.9 (IDL für CSIv2). Spec-Coverage in `docs/spec-coverage/corba-3.3.md` §10 alle auf `done`.
- (Hinweis: ältere Doku-Stelle erwähnt §24 — das ist eine Zähl-Variante in der Spec; aktuelle Spec-Coverage-Doc folgt der Part-2-§10-Numerierung.)

### Public-API

- `AssociationOptions(u16)` mit Konstanten `NO_PROTECTION`, `INTEGRITY`, `CONFIDENTIALITY`, `DETECT_REPLAY`, `DETECT_MISORDERING`, `ESTABLISH_TRUST_IN_TARGET`, `ESTABLISH_TRUST_IN_CLIENT`, `NO_DELEGATION`, `SIMPLE_DELEGATION`, `COMPOSITE_DELEGATION`, `IDENTITY_ASSERTION`, `NO_PROTECTION_LIBARY`.
- `CompoundSecMech`, `CompoundSecMechList`, `AsContextSec`, `SasContextSec`.
- `GssupCredentialToken`, `INITIAL_CONTEXT_TOKEN_TAG`.
- `SasMessage` + `EstablishContext`, `CompleteEstablishContext`, `MessageInContext`, `ContextError`, `IdentityToken`.

### Implementierung

**Algorithmen + Datenstrukturen:** `AssociationOptions` ist ein `pub struct AssociationOptions(pub u16)` mit der Bitmask aus Spec §10.6 als `pub const`-Konstanten (Caller bit-orsd / -andsd direkt auf `u16`). `CompoundSecMechList` wird im IOR als `TAG_CSI_SEC_MECH_LIST`-TaggedComponent eingebettet; CDR-Encoding via `zerodds-cdr`. `SasMessage` ist ein Variant-Enum mit den vier Spec-§10.2-Typen (EstablishContext / CompleteEstablishContext / MessageInContext / ContextError); jede Variante hat eigene Wire-Darstellung. `GssupCredentialToken` wird mit dem `INITIAL_CONTEXT_TOKEN`-Tag (Spec §10.9 Annex / GSSUP) gewrappt. `IdentityToken` deckt §10.3 Identity-Token-Form ab.

**Performance / Sicherheit / no_std:** Wire-Konformitaet: Octet-fuer-Octet pro Spec §10-CDR-Format; alle Inline-Tests sind Roundtrip-Tests gegen die Spec-Bitmuster. `#![cfg_attr(not(feature = "std"), no_std)]` + `#![forbid(unsafe_code)]` + `extern crate alloc`. Workspace-Lints `unsafe_code = forbid`, `unwrap_used`/`expect_used`/`panic` = `warn`; in Test-Modulen via `#[allow]` ausgenommen. Cross-Vendor-Interop-Pfad: Wire-Bytes-Ebene durch CDR-Roundtrip verifiziert (Live-Test gegen TAO/JacORB ist Daemon-Caller-Aufgabe).

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-cdr` (Wire-Codec).
- **Dependents (out):** GIOP-/IIOP-Server (Layer-8-Tier-B/C) mit Security-Stack-Konfiguration.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch OMG CORBA 3.3 Part 3 §24 fixiert.

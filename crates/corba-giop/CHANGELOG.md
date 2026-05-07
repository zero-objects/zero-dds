# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-giop`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 2** §15 (General Inter-ORB Protocol).
- **OMG CORBA 3.3 Part 2** §15.4 (Message-Types) + §15.4.1-§15.4.9
  (alle 8 Messages).
- **OMG CORBA 3.3 Part 2** §15.5 (Service-Context-Tags).

### Public-API

**Header + Codec:**
- `MAGIC` / `MAGIC_BYTES` — `"GIOP"`-Magic.
- `MessageHeader { magic, giop_version, flags, message_type, message_size }`.
- `Version`, `Flags`, `MessageType`.
- `Message`-Enum + `encode_message` / `decode_message`.

**Message-Types (alle 8):**
- `Request` + `ResponseFlags`.
- `Reply` + `ReplyStatusType` (alle 6 Statuses).
- `CancelRequest`, `LocateRequest`, `LocateReply` + `LocateStatusType`.
- `CloseConnection`, `MessageError`.
- `Fragment` + `FragmentHeader`.

**Service-Contexts:**
- `ServiceContext`, `ServiceContextList`, `ServiceContextTag`.

**Target-Address (GIOP 1.2):**
- `TargetAddress`-Union, `ObjectKey`.

**Errors:**
- `GiopError`, `GiopResult<T>`.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`. CDR-1-Marshalling via `zerodds-cdr`.

GIOP 1.0/1.1/1.2-Versions-Quirks korrekt abgebildet:
- 1.0: Request-Header inkl. `requesting_principal`.
- 1.1: Fragment-Flag fuer Request/Reply.
- 1.2: Header neu organisiert, `TargetAddress`-Union, 8-Byte-aligned-Body, Fragment fuer alle Types, BiDir-GIOP.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-cdr`.
- **Dependents (out):** `zerodds-corba-iiop` (Transport-Schicht), `zerodds-corba-dds-bridge` (Pass-through-Decoding).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- GIOP-Wire-Format ist durch OMG-Spec fixiert.
- Service-Context-Tags-Liste: durch IANA-OMG-Registry fixiert; Erweiterungen sind Major-Bump-Kandidaten.

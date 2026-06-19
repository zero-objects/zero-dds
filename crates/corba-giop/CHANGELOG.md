# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-giop` crate.

### Spec references

- **OMG CORBA 3.3 Part 2** §15 (General Inter-ORB Protocol).
- **OMG CORBA 3.3 Part 2** §15.4 (message types) + §15.4.1-§15.4.9
  (all 8 messages).
- **OMG CORBA 3.3 Part 2** §15.5 (service context tags).

### Public API

**Header + codec:**
- `MAGIC` / `MAGIC_BYTES` — `"GIOP"` magic.
- `MessageHeader { magic, giop_version, flags, message_type, message_size }`.
- `Version`, `Flags`, `MessageType`.
- `Message` enum + `encode_message` / `decode_message`.

**Message types (all 8):**
- `Request` + `ResponseFlags`.
- `Reply` + `ReplyStatusType` (all 6 statuses).
- `CancelRequest`, `LocateRequest`, `LocateReply` + `LocateStatusType`.
- `CloseConnection`, `MessageError`.
- `Fragment` + `FragmentHeader`.

**Service contexts:**
- `ServiceContext`, `ServiceContextList`, `ServiceContextTag`.

**Target address (GIOP 1.2):**
- `TargetAddress` union, `ObjectKey`.

**Errors:**
- `GiopError`, `GiopResult<T>`.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`. CDR-1 marshalling via `zerodds-cdr`.

GIOP 1.0/1.1/1.2 version quirks correctly mapped:
- 1.0: request header including `requesting_principal`.
- 1.1: fragment flag for request/reply.
- 1.2: header reorganized, `TargetAddress` union, 8-byte-aligned body, fragment for all types, bidirectional GIOP.

### Architecture

- **Layer:** 8 (CORBA stack, Tier A).
- **Dependencies (in):** `zerodds-cdr`.
- **Dependents (out):** `zerodds-corba-iiop` (transport layer), `zerodds-corba-dds-bridge` (pass-through decoding).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1 stable.
- The GIOP wire format is fixed by the OMG spec.
- Service context tag list: fixed by the IANA/OMG registry; extensions are major-bump candidates.

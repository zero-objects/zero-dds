# Changelog — `zerodds-rtps`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1-Audit

- License-Header (SPDX-Apache-2.0) auf alle 31 src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Spec-Verankerung-Cleanup: 54 Phase-X-Marker (`Phase-0-Scope`,
  `Phase-1-Wire`, `Phase-N D.x`, etc.) und Out-of-scope-Listen
  bereinigt — alle implementierten Submessages explizit gelistet,
  Layer-Boundaries klar dokumentiert.
- README rewrite: aktualisierte Test-Count (647), Lock-Free-Read-Path-
  Sektion ohne Phase-X-Sprache.

### Eigenschaften

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety-Klasse **SAFE**.
- 647 Tests grün; clippy clean.
- DDSI-RTPS 2.5 voll spec-konform (K3b-Audit 2026-04-28: 121 done /
  0 partial / 0 open / 3 n/a).

### Public API

- Wire-Types: `Guid`, `EntityId`, `SequenceNumber`, `Locator`,
  `ProtocolVersion`, `VendorId`.
- Submessages: DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, ACKNACK,
  NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY.
- State-Machines: `BestEffortWriter`/`Reader`, `ReliableWriter`/
  `Reader`, `ReliableStatelessWriter` (für SPDP).
- History-Cache: `HistoryCache` mit Atomic-Stats +
  `LockFreeReadHistoryCache` mit RCU-Snapshot.
- BuiltinTopicData: `ParticipantBuiltinTopicData`,
  `PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`.
- ParameterList (PL_CDR_LE) mit allen DDSI-/Security-/XTypes-PIDs.
- Fragmentation: `FragmentAssembler` mit DoS-Caps.

### Cross-Vendor-Interop

Wire-byte-identisch gegen Cyclone DDS, FastDDS, RTI Connext,
OpenSplice. Cross-Vendor-Live-Tests in `discovery`-Crate via
`--features live-interop`.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

# Changelog — `zerodds-rtps`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all 31 src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Spec-anchoring cleanup: 54 Phase-X markers (`Phase-0-Scope`,
  `Phase-1-Wire`, `Phase-N D.x`, etc.) and out-of-scope lists
  cleaned up — all implemented submessages explicitly listed,
  layer boundaries clearly documented.
- README rewrite: updated test count (647), lock-free-read-path
  section without Phase-X language.

### Features

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 647 tests green; clippy clean.
- DDSI-RTPS 2.5 fully spec-conformant (K3b audit 2026-04-28: 121 done /
  0 partial / 0 open / 3 n/a).

### Public API

- Wire types: `Guid`, `EntityId`, `SequenceNumber`, `Locator`,
  `ProtocolVersion`, `VendorId`.
- Submessages: DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, ACKNACK,
  NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY.
- State machines: `BestEffortWriter`/`Reader`, `ReliableWriter`/
  `Reader`, `ReliableStatelessWriter` (for SPDP).
- History cache: `HistoryCache` with atomic stats +
  `LockFreeReadHistoryCache` with RCU snapshot.
- BuiltinTopicData: `ParticipantBuiltinTopicData`,
  `PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`.
- ParameterList (PL_CDR_LE) with all DDSI/security/XTypes PIDs.
- Fragmentation: `FragmentAssembler` with DoS caps.

### Cross-vendor interop

Wire-byte-identical against Cyclone DDS, FastDDS, RTI Connext,
OpenSplice. Cross-vendor live tests in the `discovery` crate via
`--features live-interop`.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

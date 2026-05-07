# Changelog — `zerodds-transport-shm`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1-Audit

**Breaking** (interne Aufräumung; öffentliche Production-API
unverändert):

- Intra-Process-Stub (`ShmTransport`, `registry`, `ring_buffer` Module)
  entfernt — war Phase-1-Artefakt ohne externen Konsumenten. Production
  nutzt ausschließlich `PosixShmTransport`.

**Sonstige Änderungen**:

- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-Header rewrite: ehrliche Spec-Story (DDSI-RTPS §9.4 LocatorKind
  OMG-normativ; Segment-Layout + SpSc + Cleanup ZeroDDS-eigen).
- Internal-only Review-Cycle-Marker (`phase2-0-*`) aus posix.rs entfernt.
- ZeroDDS-SHM-Transport-1.0-Spec materialisiert in
  `docs/spec-coverage/zerodds-shm-transport-1.0.md` (§1-§8 mit
  Segment-Layout, Frame-Format, Sync-Modell, Cleanup-Semantik,
  Plattform-Support, Test-Mapping).
- README + CHANGELOG.

### Eigenschaften

- `std`-only, Safety-Klasse **STANDARD**.
- 18 Tests grün (17 lib + 1 cross-process integration); clippy clean.
- Cross-Process SHM via POSIX `shm_open` + `mmap`.
- Lock-free SpSc-Ringbuffer mit `AcqRel`-Atomics.
- Crash-Recovery via predictable `os_id` + `shm_unlink`.
- Advisory `flock`-Race-Protection beim Owner-Create.
- Shutdown-Flag für Owner→Consumer-Termination-Signaling.

### Plattform-Support

- Linux primary, macOS supported, Windows best-effort, no_std nicht
  supported.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

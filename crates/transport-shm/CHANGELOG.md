# Changelog — `zerodds-transport-shm`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

**Breaking** (internal cleanup; the public production API is
unchanged):

- Intra-process stub (`ShmTransport`, `registry`, `ring_buffer`
  modules) removed — was a phase-1 artifact with no external consumer.
  Production uses `PosixShmTransport` exclusively.

**Other changes**:

- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest spec story (DDSI-RTPS §9.4 LocatorKind
  OMG-normative; segment layout + SpSc + cleanup ZeroDDS-own).
- Internal-only review-cycle markers (`phase2-0-*`) removed from posix.rs.
- ZeroDDS SHM Transport 1.0 spec materialized in
  `docs/spec-coverage/zerodds-shm-transport-1.0.md` (§1-§8 with
  segment layout, frame format, sync model, cleanup semantics,
  platform support, test mapping).
- README + CHANGELOG.

### Features

- `std`-only, safety class **STANDARD**.
- 18 tests green (17 lib + 1 cross-process integration); clippy clean.
- Cross-process SHM via POSIX `shm_open` + `mmap`.
- Lock-free SpSc ring buffer with `AcqRel` atomics.
- Crash recovery via a predictable `os_id` + `shm_unlink`.
- Advisory `flock` race protection on owner-create.
- Shutdown flag for owner→consumer termination signaling.

### Platform support

- Linux primary, macOS supported, Windows best-effort, no_std not
  supported.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

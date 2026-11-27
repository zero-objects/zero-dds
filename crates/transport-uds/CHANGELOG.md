# Changelog — `zerodds-transport-uds`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- On Windows the close of the stream transport ends the reading of a connection whose peer holds it open: a read without a deadline waits with `WSAPoll`, at most `ACCEPT_POLL` (50 ms) at a time, and looks at the closed flag in between. The shutdown of the connection did not end a `recv` waiting on it there, and the close waited on the reading thread (five stream tests on the Windows CI runner past 25 s).
- On Windows a send of the stream transport looks first whether the peer has closed its outgoing connection (`WSAPoll` without waiting, then a peek) and connects anew if so (§3.5). The write after the peer closed succeeded there, and the frame was lost; on Linux and macOS that write fails with `EPIPE`.
- A `base_dir` created by a process that runs as `SYSTEM` gets the ACL `D:P(A;OICI;FA;;;SY)`: its user is `SYSTEM`, and Windows kept one of the two equal entries anyway.

### Added

- `Transport::max_message_size` is the datagram limit of the socket (DDSI-RTPS 2.5 §8.4.14.1).

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest spec story (DDSI-RTPS §9.4 LocatorKind
  OMG-normative; path resolution + SOCK_DGRAM ZeroDDS-own).
- Internal-only review-cycle markers (`phase2-0-*`) stripped.
- ZeroDDS UDS Transport 1.0 spec materialized in
  `docs/spec-coverage/zerodds-uds-transport-1.0.md` (§1-§9 with
  path resolution, wire format, cleanup semantics, container use case,
  platform support, test mapping).
- README + CHANGELOG.

### Features

- `std`-only, safety class **STANDARD**.
- 17 tests green (16 lib + 1 cross-process integration); clippy clean.
- SOCK_DGRAM filesystem sockets (default).
- Linux abstract-namespace support via the `abstract_dgram` module.
- TOCTOU-safe bind-path validation.
- Lazy base-directory creation with mode 0700.

### Platform support

- Linux primary (filesystem + abstract namespace).
- macOS supported (filesystem only).
- Windows not supported (Unix-specific).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

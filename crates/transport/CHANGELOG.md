# Changelog — `zerodds-transport`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Transport::max_message_size` and `DEFAULT_MAX_MESSAGE_SIZE` (1472): the largest RTPS message a transport sends in one datagram. DDSI-RTPS 2.5 §8.4.14.1 takes the fragment size of a writer from the transport with the smallest one.

## [1.0.0-rc.1] — 2026-05-05

### RC1-Audit
- License header (SPDX-Apache-2.0).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- README with an architecture note on the `transport → rtps` crate dependency.
- Crate header with spec anchor DDSI-RTPS 2.5 §8.3.2 for the Locator re-export.

### Properties
- `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 6 unit tests green; clippy clean.
- 15 cross-crate import sites (dcps, discovery, transport-udp/tcp/shm/uds, tools).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1

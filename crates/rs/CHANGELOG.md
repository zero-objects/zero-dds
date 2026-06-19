# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-rs`** crate as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
Idiomatic Rust SDK, async/await, streams

### Architecture
- Layer: 6 (PSMs / bindings)
- Dependencies (in): `zerodds-c-api` (foundation) + language-specific helper crates
- Dependents (out): application code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.


# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-ts-wasm`** crate as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
ZeroDDS WASM bindings — XCDR1/XCDR2 codec for browser/JS

### Architecture
- Layer: 6 (PSMs / bindings)
- Dependencies (in): `zerodds-c-api` (foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.


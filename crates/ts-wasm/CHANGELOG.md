# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Removed

- **`src/cdr/`** — a hand-written TypeScript XCDR2 codec
  (`Xcdr2Writer`/`Xcdr2Reader`/`md5`/`DdsTopicType`), re-exported from
  `index.ts` as `@zerodds/wasm`'s own codec surface but not used by
  anything: `zerodds-idlc generate --lang typescript` imports from
  `@zerodds/wire` (`runtime/typescript`) directly and always has, and no
  other file in this crate ever imported `./cdr`. Its own module
  comment named `crates/ts-node/src/cdr/` as its byte-identical twin;
  that directory was already removed (`ts-node: remove the duplicate
  XCDR2 codec, depend on runtime/typescript`) — this one was not, until
  now.
  - It was also wrong where it differed from `@zerodds/wire`: for a
    variable-length `@mutable` member, `writer.ts` wrote XTypes 1.3
    §7.4.3.4.2's EMHEADER length code 3 with a NEXTINT. Length code 3
    is 8 inline octets and no NEXTINT at all; a conformant peer reading
    that header consumes exactly 8 octets and desyncs from that member
    on. It also claimed the EMHEADER word is written big-endian
    "independent of the stream endian", where `runtime/cpp`'s
    `zerodds::wire::Writer` (and `crates/wire`) put it in the ambient
    stream order per the same clause. Found by inspection while
    removing the file, not by a failing test — this crate carried no
    wire-vector test of its own to catch it.
  - `CdrEncoder`/`CdrDecoder` (`src/lib.rs`, wasm-bindgen over
    `zerodds-wire`'s `Writer`/`Reader`) are unaffected and remain the
    one codec this package ships; they never duplicated `src/cdr/`'s
    logic, only its purpose.
  - Dropped with it: the "KeyHash computation (XTypes 1.3 §7.6.8)"
    entry in `README.md`'s scope list — `src/cdr/md5.ts` was the only
    place that lived. `zerodds-wire` has no MD5 to wrap, so this is an
    open gap, left open rather than reintroduced as a local copy (the
    same call `runtime/typescript`'s own MD5 gap made).

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


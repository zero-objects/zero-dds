# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-foundation` crate.

### Spec references

- **RFC 4960 Appendix B** — CRC-32C (Castagnoli) for SCTP, used here as a wire-integrity hash for the DDSI-RTPS HEADER_EXTENSION `messageChecksum` (DDSI-RTPS 2.5 §9.4.2.15.2).
- **ECMA-182 / XZ utils** — CRC-64-XZ as a second variant of the same HEADER_EXTENSION checksum.
- **RFC 1321** — MD5-128 as a third variant; additionally used for the XTypes 1.3 EquivalenceHash (§7.3.1.2.1), NameHash (§7.3.4.5) and KeyHash (§7.6.8.4 Step 5.2), as well as for the RTPS GroupDigest_t (§8.3.5.10).
- **OpenTelemetry Spans** — the `TraceId`/`SpanId`/`SpanKind`/`SpanStatus` model matches the OpenTelemetry specification and is exportable via `zerodds-observability-otlp`.

### Public API

**Stack buffer:**
- `PoolBuffer<CAP>` — fixed-capacity on-stack buffer with `extend_from_slice`, `push`, `as_slice`, `clear`.
- `PoolBufferError` — `Overflow`, `CapacityTooLarge`.

**Wire-integrity hashes:**
- `crc32c(&[u8]) -> u32` — RFC 4960 Appendix B.
- `crc64_xz(&[u8]) -> u64` — ECMA-182 / XZ utils.
- `md5(&[u8]) -> [u8; 16]` — RFC 1321.

**Observability:**
- `Event`, `Level`, `Component`, `Attribute` — structured event language.
- `Sink` (trait), `NullSink`, `StderrJsonSink`, `VecSink`, `SharedSink`, `null_sink()` — sink family.

**Tracing:**
- `Span`, `SpanContext`, `SpanId`, `TraceId`, `SpanKind`, `SpanStatus`.
- `Histogram` — coarse-grained latency/throughput recording.

**RCU:**
- `RcuCell<T>` — copy-on-write container with `Arc<T>` snapshots, without `unsafe`.

### Implementation

CRC lookup tables are built with `const fn` (1 KiB for CRC-32C, 2 KiB for CRC-64), with no runtime initialization. MD5 follows RFC 1321 §3 + Appendix A directly; in `alloc` mode with Vec padding for arbitrary input lengths, in strict `no_std` mode limited to 56 bytes (a single 64-byte block without padding overflow). All three hashes are validated against their RFC and ECMA test vectors.

`PoolBuffer<CAP>` models the length counter as a `u16` (maximum 65535 bytes) and returns `CapacityTooLarge` for `CAP > u16::MAX`. `RcuCell<T>` protects the reference cell with a `Mutex<Arc<T>>` — readers access an Arc clone and work lock-free, writers do copy-on-write. The trade-off against unsafe AtomicPtr performance is deliberate in favor of strictly safe code.

Pure Rust, `forbid(unsafe_code)`, no external crates. Hot-path algorithms are implemented without touching the heap.

### Architecture

- **Layer:** 0 (Foundation).
- **Dependencies (in):** none — Foundation is the base layer.
- **Dependents (out):** `zerodds-cdr` (md5 for KeyHash), `zerodds-types` (md5 for EquivalenceHash + NameHash), `zerodds-rtps` (md5 for GroupDigest, RcuCell for HistoryCache, crc32c/crc64_xz/md5 for the HEADER_EXTENSION checksum, PoolBuffer for hot-path encoding), `zerodds-dcps` (PoolBuffer for small-frame encoding, observability for event sinks), `zerodds-observability-otlp` (Event, Tracing).
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserved).

### Stability

All `pub` items are RC1-stable; breaking changes require a major bump to `2.0.0`. No `unstable-` modules.

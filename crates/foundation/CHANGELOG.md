# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-foundation`-Crate.

### Spec-Referenzen

- **RFC 4960 Appendix B** — CRC-32C (Castagnoli) für SCTP, hier als Wire-Integrity-Hash für DDSI-RTPS HEADER_EXTENSION `messageChecksum` (DDSI-RTPS 2.5 §9.4.2.15.2).
- **ECMA-182 / XZ utils** — CRC-64-XZ als zweite Variante derselben HEADER_EXTENSION-Checksum.
- **RFC 1321** — MD5-128 als dritte Variante; zusätzlich genutzt für XTypes 1.3 EquivalenceHash (§7.3.1.2.1), NameHash (§7.3.4.5) und KeyHash (§7.6.8.4 Step 5.2), sowie für RTPS GroupDigest_t (§8.3.5.10).
- **OpenTelemetry Spans** — `TraceId`/`SpanId`/`SpanKind`/`SpanStatus`-Modell entspricht der OpenTelemetry-Spezifikation und ist via `zerodds-observability-otlp` exportierbar.

### Public-API

**Stack-Buffer:**
- `PoolBuffer<CAP>` — fixed-capacity on-stack Buffer mit `extend_from_slice`, `push`, `as_slice`, `clear`.
- `PoolBufferError` — `Overflow`, `CapacityTooLarge`.

**Wire-Integrity-Hashes:**
- `crc32c(&[u8]) -> u32` — RFC 4960 Appendix B.
- `crc64_xz(&[u8]) -> u64` — ECMA-182 / XZ utils.
- `md5(&[u8]) -> [u8; 16]` — RFC 1321.

**Observability:**
- `Event`, `Level`, `Component`, `Attribute` — strukturierte Event-Sprache.
- `Sink` (Trait), `NullSink`, `StderrJsonSink`, `VecSink`, `SharedSink`, `null_sink()` — Sink-Familie.

**Tracing:**
- `Span`, `SpanContext`, `SpanId`, `TraceId`, `SpanKind`, `SpanStatus`.
- `Histogram` — grobgranulare Latenz-/Throughput-Aufzeichnung.

**RCU:**
- `RcuCell<T>` — Copy-on-Write-Container mit `Arc<T>`-Snapshots, ohne `unsafe`.

### Implementierung

CRC-Lookup-Tables sind `const fn`-konstruiert (1 KiB für CRC-32C, 2 KiB für CRC-64), keine Runtime-Initialisierung. MD5 folgt RFC 1321 §3 + Anhang A direkt; im `alloc`-Modus mit Vec-Padding für beliebige Eingabelängen, im strikten `no_std`-Modus auf 56 Byte limitiert (single 64-Byte-Block ohne Padding-Overflow). Alle drei Hashes sind gegen ihre RFC- bzw. ECMA-Test-Vektoren validiert.

`PoolBuffer<CAP>` modelliert den Length-Counter als `u16` (Maximum 65535 Byte) und gibt `CapacityTooLarge` für `CAP > u16::MAX` zurück. `RcuCell<T>` schützt die Reference-Cell mit einem `Mutex<Arc<T>>` — Reader greifen auf einen Arc-Klon zu und arbeiten lock-free, Writer machen Copy-on-Write. Trade gegen unsafe-AtomicPtr-Performance ist bewusst zugunsten von strikt-safe-Code.

Pure-Rust, `forbid(unsafe_code)`, keine externen Crates. Hot-Path-Algorithmen sind ohne Heap-Touch implementiert.

### Architektur

- **Layer:** 0 (Foundation).
- **Dependencies (in):** keine — Foundation ist die Basis-Schicht.
- **Dependents (out):** `zerodds-cdr` (md5 für KeyHash), `zerodds-types` (md5 für EquivalenceHash + NameHash), `zerodds-rtps` (md5 für GroupDigest, RcuCell für HistoryCache, crc32c/crc64_xz/md5 für HEADER_EXTENSION-Checksum, PoolBuffer für Hot-Path-Encoding), `zerodds-dcps` (PoolBuffer für Small-Frame-Encoding, observability für Event-Sinks), `zerodds-observability-otlp` (Event, Tracing).
- **Feature-Flags:** `std` (default), `alloc` (via std), `safety` (reserviert).

### Stabilität

Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump auf `2.0.0`. Keine `unstable-`-Module.

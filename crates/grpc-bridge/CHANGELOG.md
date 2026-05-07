# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-grpc-bridge`-Crate.

### Spec-Referenzen

- **gRPC HTTP/2 Protocol** (`grpc.io/docs/what-is-grpc/core-concepts/`): Length-Prefixed Message, `:path`-Format `/<service>/<method>`, `grpc-timeout`-Header (H/M/S/m/u/n), `grpc-status` Status-Codes (0..=16), Custom-Metadata (`-bin`-Suffix → Base64).
- **gRPC-Web Specification**: Trailer-Frame (LPM mit Compressed-Flag-MSB=1), Content-Types (`application/grpc-web`, `application/grpc-web+proto`, `application/grpc-web-text`, `application/grpc-web+json`).

### Public-API

**Frame (`frame`-Modul):**
- `encode_message(payload: &[u8], compressed: bool) -> Result<Vec<u8>, FrameError>`.
- `decode_message(bytes: &[u8]) -> Result<(u8 /* flag */, Vec<u8>, usize), FrameError>`.
- `FrameError`.

**Path (`path`-Modul):**
- `parse_path(path: &str) -> Result<(&str /* service */, &str /* method */), PathError>`.
- `PathError`.

**Timeout (`timeout`-Modul):**
- `TimeoutUnit::{Hours, Minutes, Seconds, Milliseconds, Microseconds, Nanoseconds}`.
- `encode_timeout(value: u64, unit: TimeoutUnit) -> Result<String, TimeoutError>`.
- `decode_timeout(s: &str) -> Result<(u64, TimeoutUnit), TimeoutError>`.

**Status (`status`-Modul):**
- `Status::{Ok, Cancelled, Unknown, InvalidArgument, DeadlineExceeded, NotFound, AlreadyExists, PermissionDenied, ResourceExhausted, FailedPrecondition, Aborted, OutOfRange, Unimplemented, Internal, Unavailable, DataLoss, Unauthenticated}` (alle 17 §"Status"-Codes).

**Metadata (`metadata`-Modul):**
- `BIN_SUFFIX = "-bin"`.
- `is_binary_header(name) -> bool`.
- `encode_header_value(value: &[u8], binary: bool) -> Result<String, MetadataError>` / `decode_header_value(name, value) -> Result<Vec<u8>, MetadataError>`.
- `encode_base64` / `decode_base64`.
- `request_headers(...)` / `response_headers(...)` — Standard-Header-Sets (incl. `:method=POST`, `content-type`, `te=trailers`).
- `content_types::{GRPC, GRPC_PROTO, GRPC_WEB, GRPC_WEB_PROTO, GRPC_WEB_TEXT, GRPC_WEB_JSON}`.
- `MetadataError`.

**Server (`server`-Modul):**
- `GrpcRequest { service, method, headers, body }`.
- `GrpcResponse { status, headers, body }`.
- `GrpcServer::{new, dispatch}` — Skeleton fuer Caller-konfigurierte HTTP/2-Listener; Caller hookt seine eigene Dispatch-Logik per Trait-Closure.

### Implementierung

`encode_message` / `decode_message` macht das LPM-Wire-Format strict per Spec: `[flag: u8, length: u32 BE, bytes...]`. `flag & 0x80` signalisiert gRPC-Web-Trailer-Frame; `flag & 0x01` signalisiert Compressed-Body. Decoder rejected wenn `bytes.len() < 5 + length`.

`parse_path` lehnt nicht-`/<svc>/<method>`-Form ab (z.B. fehlender Slash, Wrong-Anzahl-Slashes, leere Komponenten).

`encode_timeout` waehlt automatisch die kompaktesten Unit-Bytes; `decode_timeout` akzeptiert alle Standard-Units. Spec gibt 8-stelliges Maximum vor (z.B. `99999999H`); Encoder rejected ueberlange Werte.

Custom-Metadata: `is_binary_header` checkt nur das `-bin`-Suffix (case-insensitive). `encode_header_value` macht Base64 wenn binary, sonst raw-ASCII (rejected non-printable). `decode_header_value` hebt das Base64-Encoding wieder auf.

`GrpcServer` ist absichtlich ein Skeleton — die Crate gibt **kein** TCP/TLS/HTTP/2-Connection-Management mit (das uebernimmt Caller via `zerodds-http2`); sie liefert nur die Application-Layer-Dispatch-Hooks.

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** `zerodds-http2` (Framing + Streams), `zerodds-hpack` (HEADERS-Compression).
- **Dependents (out):** (vorgesehen) DDS-RPC-Service-Endpoint-Layer.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch gRPC HTTP/2 Protocol + gRPC-Web fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: TLS-Server (rustls 0.23) mit ALPN-`h2` + Auth-Modes
  + Topic-ACL via `zerodds-bridge-security` (Bridge-Spec §7.1/§7.2/§7.3).
- Service-Codegen-Helper (`service_gen.rs`) +
  Reflection-API-Modul (`reflection.rs`) +
  Cross-Vendor-Interop-Modul.
- DDS-QoS → gRPC-Behavior-Translation in `qos_translation`.
